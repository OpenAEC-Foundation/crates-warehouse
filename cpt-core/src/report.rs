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
use crate::plot::render_cpt_svg;

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

    let cover = Some(Cover {
        subtitle: Some(project.location.clone()),
        image: None,
        extra_fields,
    });

    let mut sections: Vec<Section> = Vec::with_capacity(cpts.len() + 1);

    // 1. Coordinate table section
    sections.push(coord_table_section(cpts));

    // 2. One page per CPT
    for cpt in cpts {
        sections.push(cpt_page_section(cpt));
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
            let (x, y, z) = match cpt.position {
                Some(p) => (
                    Value::from(round2(p.x_rd)),
                    Value::from(round2(p.y_rd)),
                    p.z_nap
                        .map(|v| Value::from(round2(v)))
                        .unwrap_or(Value::Null),
                ),
                None => (Value::Null, Value::Null, Value::Null),
            };

            let max_depth = cpt_max_depth(cpt);
            let datum = cpt
                .metadata
                .date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            vec![
                Value::from(cpt.id.clone()),
                x,
                y,
                z,
                Value::from(round2(max_depth)),
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

/// Build a per-CPT page: caption paragraph + embedded SVG chart.
fn cpt_page_section(cpt: &Cpt) -> Section {
    let svg = render_cpt_svg(cpt);
    let svg_b64 = base64_encode(svg.as_bytes());

    let caption = format!(
        "Sondering {} — maximale diepte {:.2} m{}",
        cpt.id,
        cpt_max_depth(cpt),
        match cpt.metadata.date {
            Some(d) => format!(", uitgevoerd {}", d.format("%Y-%m-%d")),
            None => String::new(),
        }
    );

    let image = ImageBlock {
        src: ImageSource::Base64 {
            data: svg_b64,
            media_type: MediaType::Svg,
            filename: Some(format!("{}.svg", cpt.id)),
        },
        caption: Some(caption.clone()),
        width_mm: Some(170.0),
        alignment: Alignment::Center,
    };

    Section {
        title: format!("Sondering {}", cpt.id),
        level: 1,
        content: vec![
            ContentBlock::Paragraph(ParagraphBlock {
                text: caption,
                style: "Normal".to_string(),
            }),
            ContentBlock::Image(image),
        ],
        orientation: None,
        page_break_before: true,
    }
}

/// Largest measured depth in a CPT (m below ground level). Returns 0.0 for an
/// empty CPT — caller may use this defensively.
fn cpt_max_depth(cpt: &Cpt) -> f64 {
    cpt.points.iter().map(|p| p.depth).fold(0.0_f64, f64::max)
}

/// Round to 2 decimals — table presentation only.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
