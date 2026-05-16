//! Report builder — produces an [`openaec_core::schema::ReportData`] from one
//! or more parsed [`Cpt`]s plus project metadata. PDF rendering itself happens
//! downstream in `openaec-engine`.
//!
//! The output has three logical parts:
//! 1. **Cover** — project metadata (subtitle = location, extra fields).
//! 2. **Coordinate table** — one section with a `TableBlock` listing every CPT
//!    with X-RD, Y-RD, Z-NAP, end-depth and date.
//! 3. **Per-CPT pages** — one section per CPT containing a caption paragraph
//!    and the SVG chart (Task 11) embedded as a base64 image.

use std::collections::HashMap;

use openaec_core::schema::{
    Alignment, ContentBlock, Cover, ImageBlock, ImageSource, MediaType, Orientation,
    PaperFormat, ParagraphBlock, ReportData, ReportStatus, Section, TableBlock, TableStyle,
};
use serde_json::Value;

use crate::domain::Cpt;
use crate::plot::render_cpt_png_with_meta;

/// Project-level metadata supplied by the user / Tauri frontend.
///
/// Maps onto `ReportData::project`, `client`, `project_number`, `author`,
/// `date` plus the cover's `subtitle` / extra fields.
#[derive(Debug, Clone)]
pub struct ProjectMeta {
    pub title: String,
    pub client: String,
    pub location: String,
    pub project_number: String,
    pub author: String,
    pub date: chrono::NaiveDate,
}

/// Build a [`ReportData`] from a slice of CPTs and project metadata.
///
/// The CPTs are listed in input order in the coordinate table and rendered to
/// pages in the same order. SVG plots are embedded inline as base64 so the
/// `openaec-engine` rasterizer (`resvg`) can pick them up without any
/// file-system round-trip.
pub fn build(cpts: &[Cpt], project: &ProjectMeta) -> ReportData {
    let date_iso = project.date.format("%Y-%m-%d").to_string();

    // Cover with subtitle = location, project metadata in extra_fields.
    let mut extra_fields = HashMap::new();
    extra_fields.insert("Opdrachtgever".to_string(), project.client.clone());
    extra_fields.insert("Locatie".to_string(), project.location.clone());
    extra_fields.insert("Projectnummer".to_string(), project.project_number.clone());
    extra_fields.insert("Datum".to_string(), date_iso.clone());

    // If a single CPT is requested we skip the cover and coordinate table
    // so the CPT plot is the *only* page — matching the reference layout.
    // For multi-CPT builds we keep the existing cover + table for context.
    let single = cpts.len() == 1;

    let cover = if single { None } else {
        Some(Cover {
            subtitle: Some(project.location.clone()),
            image: None,
            extra_fields,
        })
    };

    let mut sections: Vec<Section> = Vec::with_capacity(cpts.len() + 1);

    // 1. Coordinate table section (only when more than one CPT)
    if !single {
        sections.push(coord_table_section(cpts));
    }

    // 2. One page per CPT
    for cpt in cpts {
        sections.push(cpt_page_section(cpt, project));
    }

    ReportData {
        template: "openaec.cpt".to_string(),
        project: project.title.clone(),
        tenant: None,
        format: PaperFormat::A4,
        orientation: Orientation::Portrait,
        project_number: Some(project.project_number.clone()),
        client: Some(project.client.clone()),
        author: project.author.clone(),
        date: Some(date_iso),
        version: "1.0".to_string(),
        status: ReportStatus::Concept,
        cover,
        colofon: None,
        toc: None,
        sections,
        backcover: None,
        metadata: HashMap::new(),
    }
}

/// Build the coordinate table: one header row + one row per CPT.
///
/// Columns: Sondering · X-RD · Y-RD · Z-NAP · Diepte tot · Datum.
fn coord_table_section(cpts: &[Cpt]) -> Section {
    let headers = vec![
        "Sondering".to_string(),
        "X-RD [m]".to_string(),
        "Y-RD [m]".to_string(),
        "Z-NAP [m]".to_string(),
        "Diepte tot [m]".to_string(),
        "Datum".to_string(),
    ];

    let rows: Vec<Vec<Value>> = cpts
        .iter()
        .map(|cpt| {
            // Position cells — empty string when missing so the table still
            // lays out evenly. Strings (not JSON numbers) so trailing zeros
            // are preserved in the rendered text ("132782.50" stays).
            let (x_cell, y_cell) = match cpt.position {
                Some(p) => (fmt_coord(p.x_rd), fmt_coord(p.y_rd)),
                None => (Value::from(""), Value::from("")),
            };

            // Z-NAP — prefer position.z_nap, fall back to the metadata
            // ground level (older GEFs leak it through `#ZID` only).
            // Sign is preserved verbatim: a ground level below NAP shows
            // as `-1.06`, above NAP as `1.06`.
            let z_value: Option<f64> = cpt
                .position
                .and_then(|p| p.z_nap)
                .or(cpt.metadata.ground_level_nap);
            let z_cell = match z_value {
                Some(z) => fmt_depth(z),
                None => Value::from(""),
            };

            // Diepte tot — max measured depth across all points. Always a
            // positive value in the source data.
            let max_depth = cpt_max_depth(cpt);

            // Datum — Dutch convention dd-mm-yyyy.
            let datum = cpt
                .metadata
                .date
                .map(|d| d.format("%d-%m-%Y").to_string())
                .unwrap_or_default();

            vec![
                Value::from(cpt.id.clone()),
                x_cell,
                y_cell,
                z_cell,
                fmt_depth(max_depth),
                Value::from(datum),
            ]
        })
        .collect();

    let table = TableBlock {
        title: Some("Sonderingen — coordinaten en bereik".to_string()),
        headers,
        rows,
        column_widths: None,
        style: TableStyle::Striped,
    };

    Section {
        title: "Overzicht sonderingen".to_string(),
        level: 1,
        content: vec![
            ContentBlock::Paragraph(ParagraphBlock {
                text: "De onderstaande tabel toont de positie (RD-stelsel), \
                       maaiveldhoogte ten opzichte van NAP en de eindiepte \
                       van iedere sondering in dit rapport."
                    .to_string(),
                style: "Normal".to_string(),
            }),
            ContentBlock::Table(table),
        ],
        orientation: None,
        page_break_before: false,
    }
}

/// Build a per-CPT page: a single full-page rasterised chart image.
fn cpt_page_section(cpt: &Cpt, project: &ProjectMeta) -> Section {
    let png = render_cpt_png_with_meta(cpt, Some(&project.project_number), Some(&project.client));
    let png_b64 = base64_encode(&png);

    let image = ImageBlock {
        src: ImageSource::Base64 {
            data: png_b64,
            media_type: MediaType::Png,
            filename: Some(format!("{}.png", cpt.id)),
        },
        caption: None,
        // Hint at full A4 width; the engine's content frame (~170mm) will
        // clip it but at the largest possible width.
        width_mm: Some(200.0),
        alignment: Alignment::Center,
    };

    Section {
        // Empty title — the SVG carries its own labels.
        title: String::new(),
        level: 1,
        content: vec![ContentBlock::Image(image)],
        orientation: None,
        // No page-break — the chart needs to start on the *first* content page,
        // not the second, so the engine doesn't waste a near-empty page.
        page_break_before: false,
    }
}

/// Largest measured depth in a CPT (m below ground level). Returns 0.0 for an
/// empty CPT — caller may use this defensively.
fn cpt_max_depth(cpt: &Cpt) -> f64 {
    cpt.points.iter().map(|p| p.depth).fold(0.0_f64, f64::max)
}

/// Format an RD coordinate as a fixed two-decimal string (e.g. "132782.52").
/// Emitting a string instead of a JSON number keeps trailing zeros (serde
/// would render `132782.50` as `"132782.5"` otherwise).
fn fmt_coord(v: f64) -> Value {
    Value::from(format!("{:.2}", v))
}

/// Format a depth / elevation in metres with two decimals, preserving the
/// sign (so `-1.06` stays `-1.06` for ground-below-NAP).
fn fmt_depth(v: f64) -> Value {
    Value::from(format!("{:.2}", v))
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
