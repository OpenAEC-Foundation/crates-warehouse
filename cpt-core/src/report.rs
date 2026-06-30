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
/// Welke onderdelen het rapport bevat. Default = alles aan behalve de
/// (optionele) SBT-legenda en metadata-bijlage. Wordt door de frontend
/// (sectie-checkboxes) doorgegeven zodat aan/uit-zetten écht doorwerkt
/// in de PDF.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReportSections {
    pub cover: bool,
    pub coord_table: bool,
    pub map: bool,
    pub per_cpt: bool,
    pub sbt_legend: bool,
    pub metadata: bool,
}

impl Default for ReportSections {
    fn default() -> Self {
        Self {
            cover: true,
            coord_table: true,
            map: true,
            per_cpt: true,
            sbt_legend: false,
            metadata: false,
        }
    }
}

/// Basiskaart-afbeelding (PDOK-luchtfoto) + de RD-bounding-box waarvoor hij is
/// opgehaald. cpt-core heeft géén netwerk; de app haalt de kaart op en geeft
/// 'm hier mee. De overzichtskaart legt de sondeerlocaties exact op deze bbox.
/// Ontbreekt de kaart (offline / geen posities), dan valt de overzichtskaart
/// terug op een kaal RD-raster.
#[derive(Debug, Clone)]
pub struct OverviewBasemap {
    /// Ruwe afbeeldingsbytes (JPEG of PNG).
    pub image_bytes: Vec<u8>,
    /// MIME-type van `image_bytes`, bv. "image/jpeg".
    pub mime: String,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// Backwards-compatibele wrapper — bouwt met alle standaard-secties.
pub fn build(cpts: &[Cpt], project: &ProjectMeta) -> ReportData {
    build_with_sections(cpts, project, ReportSections::default(), None)
}

/// Bouw een [`ReportData`] met expliciete sectie-selectie. Elke ingeschakelde
/// sectie wordt in vaste volgorde toegevoegd: voorblad → coördinatentabel →
/// overzichtskaart → per-sondering → SBT-legenda → metadata. `basemap` (mag
/// `None` zijn) levert de echte luchtfoto-achtergrond voor de overzichtskaart.
pub fn build_with_sections(
    cpts: &[Cpt],
    project: &ProjectMeta,
    sec: ReportSections,
    basemap: Option<&OverviewBasemap>,
) -> ReportData {
    let date_iso = project.date.format("%Y-%m-%d").to_string();

    let mut extra_fields = HashMap::new();
    extra_fields.insert("Opdrachtgever".to_string(), project.client.clone());
    extra_fields.insert("Locatie".to_string(), project.location.clone());
    extra_fields.insert("Projectnummer".to_string(), project.project_number.clone());
    extra_fields.insert("Datum".to_string(), date_iso.clone());

    let cover = if sec.cover {
        Some(Cover {
            subtitle: Some(project.location.clone()),
            image: None,
            extra_fields,
        })
    } else {
        None
    };

    let mut sections: Vec<Section> = Vec::new();
    if sec.coord_table {
        sections.push(coord_table_section(cpts));
    }
    if sec.map {
        sections.push(overview_map_section(cpts, basemap));
    }
    if sec.per_cpt {
        for cpt in cpts {
            sections.push(cpt_page_section(cpt, project));
        }
    }
    if sec.sbt_legend {
        sections.push(sbt_legend_section());
    }
    if sec.metadata {
        sections.push(metadata_section(cpts));
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

// ─── Overzichtskaart (sondeerlocaties op RD-coördinaten) ─────────────

/// Schematische overzichtskaart: alle sondeerlocaties op hun RD-coördinaten
/// met label. Geen basemap-tegels (die vergen netwerk + compositie) — een
/// schone, zelfstandige positie-plot die de onderlinge ligging toont,
/// noord boven. Gerasteriseerd naar PNG en als afbeelding ingebed.
fn overview_map_section(cpts: &[Cpt], basemap: Option<&OverviewBasemap>) -> Section {
    let svg = overview_map_svg(cpts, basemap);
    let content = match crate::plot::rasterize_svg_to_png(&svg, 1400) {
        Some(png) => ContentBlock::Image(ImageBlock {
            src: ImageSource::Base64 {
                data: base64_encode(&png),
                media_type: MediaType::Png,
                filename: Some("overzichtskaart.png".to_string()),
            },
            caption: Some("Overzicht sondeerlocaties (RD-coördinaten, noord boven)".to_string()),
            width_mm: Some(150.0),
            alignment: Alignment::Center,
        }),
        None => ContentBlock::Paragraph(ParagraphBlock {
            text: "Geen locatiegegevens beschikbaar voor de overzichtskaart.".to_string(),
            style: "Normal".to_string(),
        }),
    };
    Section {
        title: "Overzichtskaart".to_string(),
        level: 1,
        content: vec![content],
        orientation: None,
        page_break_before: true,
    }
}

pub(crate) fn overview_map_svg(cpts: &[Cpt], basemap: Option<&OverviewBasemap>) -> String {
    use base64::Engine as _;

    let pts: Vec<(f64, f64, String)> = cpts
        .iter()
        .filter_map(|c| c.position.map(|p| (p.x_rd, p.y_rd, c.id.clone())))
        .collect();

    let w = 820.0_f64;
    let h = 820.0_f64;
    let m = 72.0_f64;

    if pts.is_empty() {
        return format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"140\" \
             viewBox=\"0 0 {w} 140\"><text x=\"24\" y=\"72\" font-family=\"Arial\" \
             font-size=\"18\" fill=\"#666\">Geen locatiegegevens beschikbaar voor de \
             overzichtskaart.</text></svg>"
        );
    }

    let plot_w = w - 2.0 * m;
    let plot_h = h - 2.0 * m;

    // Bounding-box. Mét basiskaart: exact die van de opgehaalde luchtfoto
    // (zo liggen de stippen op de juiste pixel). Zonder: een vierkante box
    // rond de posities met marge (kaal RD-raster als fallback).
    let (xmin, xmax, ymin, ymax) = if let Some(bm) = basemap {
        (bm.x_min, bm.x_max, bm.y_min, bm.y_max)
    } else {
        let xmin0 = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let xmax0 = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let ymin0 = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let ymax0 = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let span = (xmax0 - xmin0).max(ymax0 - ymin0).max(10.0);
        let cx = (xmin0 + xmax0) / 2.0;
        let cy = (ymin0 + ymax0) / 2.0;
        (cx - span * 0.6, cx + span * 0.6, cy - span * 0.6, cy + span * 0.6)
    };

    let sx = |x: f64| m + (x - xmin) / (xmax - xmin) * plot_w;
    // y omkeren zodat noord (hoge Y-RD) bovenin staat.
    let sy = |y: f64| h - m - (y - ymin) / (ymax - ymin) * plot_h;

    let mut b = String::new();
    if let Some(bm) = basemap {
        // Echte basiskaart (PDOK-luchtfoto) als achtergrond, exact passend op
        // het plot-kader. preserveAspectRatio="none" mag want de bbox is al
        // vierkant gekozen (zie de app-side fetch). Plus een dun kader.
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bm.image_bytes);
        b.push_str(&format!(
            "<image x=\"{m}\" y=\"{m}\" width=\"{plot_w}\" height=\"{plot_h}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{b64}\"/>",
            mime = bm.mime,
        ));
        b.push_str(&format!(
            "<rect x=\"{m}\" y=\"{m}\" width=\"{plot_w}\" height=\"{plot_h}\" fill=\"none\" stroke=\"#444\" stroke-width=\"1.2\"/>"
        ));
    } else {
        // Geen basiskaart → kaal RD-raster (offline-fallback).
        b.push_str(&format!(
            "<rect x=\"{m}\" y=\"{m}\" width=\"{plot_w}\" height=\"{plot_h}\" fill=\"#fafaf9\" stroke=\"#999\" stroke-width=\"1\"/>"
        ));
        for i in 1..4 {
            let gx = m + i as f64 * plot_w / 4.0;
            let gy = m + i as f64 * plot_h / 4.0;
            let y2 = h - m;
            let x2 = w - m;
            b.push_str(&format!("<line x1=\"{gx}\" y1=\"{m}\" x2=\"{gx}\" y2=\"{y2}\" stroke=\"#e7e5e4\"/>"));
            b.push_str(&format!("<line x1=\"{m}\" y1=\"{gy}\" x2=\"{x2}\" y2=\"{gy}\" stroke=\"#e7e5e4\"/>"));
        }
    }
    // Noord-pijl rechtsboven, met een wit halo-rondje zodat hij ook op een
    // donkere luchtfoto leesbaar blijft.
    let nx = w - m - 22.0;
    let ny = m + 30.0;
    b.push_str(&format!(
        "<g transform=\"translate({nx},{ny})\"><circle cx=\"0\" cy=\"8\" r=\"26\" fill=\"#FFFFFF\" fill-opacity=\"0.72\"/><line x1=\"0\" y1=\"24\" x2=\"0\" y2=\"-10\" stroke=\"#333\" stroke-width=\"2\"/><polygon points=\"0,-17 -5,-6 5,-6\" fill=\"#333\"/><text x=\"0\" y=\"40\" font-family=\"Arial\" font-size=\"13\" fill=\"#333\" text-anchor=\"middle\">N</text></g>"
    ));
    for (x, y, id) in &pts {
        let px = sx(*x);
        let py = sy(*y);
        let lx = px + 10.0;
        let ly = py + 4.0;
        let label = xml_escape(id);
        // Witte rand om de stip + wit halo achter het label, zodat beide
        // scherp afsteken tegen de luchtfoto.
        b.push_str(&format!("<circle cx=\"{px}\" cy=\"{py}\" r=\"6.5\" fill=\"#D97706\" stroke=\"#FFFFFF\" stroke-width=\"2.2\"/>"));
        b.push_str(&format!("<text x=\"{lx}\" y=\"{ly}\" font-family=\"Arial\" font-size=\"13\" font-weight=\"bold\" fill=\"#FFFFFF\" stroke=\"#FFFFFF\" stroke-width=\"3.2\" paint-order=\"stroke\">{label}</text>"));
        b.push_str(&format!("<text x=\"{lx}\" y=\"{ly}\" font-family=\"Arial\" font-size=\"13\" font-weight=\"bold\" fill=\"#27272a\">{label}</text>"));
    }
    let ry = h - m + 24.0;
    b.push_str(&format!(
        "<text x=\"{m}\" y=\"{ry}\" font-family=\"Arial\" font-size=\"11\" fill=\"#777\">X-RD {xmin:.0}–{xmax:.0} m · Y-RD {ymin:.0}–{ymax:.0} m</text>"
    ));

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"><rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#FFFFFF\"/><text x=\"{m}\" y=\"44\" font-family=\"Arial\" font-size=\"22\" font-weight=\"bold\" fill=\"#27272a\">Overzichtskaart sondeerlocaties</text>{b}</svg>"
    )
}

// ─── Robertson SBT-legenda (9 grondsoort-zones) ──────────────────────

fn sbt_legend_section() -> Section {
    let svg = sbt_legend_svg();
    let content = match crate::plot::rasterize_svg_to_png(&svg, 1000) {
        Some(png) => ContentBlock::Image(ImageBlock {
            src: ImageSource::Base64 {
                data: base64_encode(&png),
                media_type: MediaType::Png,
                filename: Some("sbt-legenda.png".to_string()),
            },
            caption: Some("Robertson grondsoort-classificatie (SBT) — kleurcodering in de sondeer-grafieken".to_string()),
            width_mm: Some(130.0),
            alignment: Alignment::Center,
        }),
        None => ContentBlock::Paragraph(ParagraphBlock {
            text: "SBT-legenda kon niet worden gerenderd.".to_string(),
            style: "Normal".to_string(),
        }),
    };
    Section {
        title: "Robertson SBT — grondsoort-legenda".to_string(),
        level: 1,
        content: vec![content],
        orientation: None,
        page_break_before: true,
    }
}

pub(crate) fn sbt_legend_svg() -> String {
    let zones = crate::robertson::zones();
    let row_h = 46.0_f64;
    let w = 560.0_f64;
    let top = 70.0_f64;
    let h = top + zones.len() as f64 * row_h + 20.0;
    let mut b = String::new();
    b.push_str(&format!(
        "<text x=\"24\" y=\"40\" font-family=\"Arial\" font-size=\"22\" font-weight=\"bold\" fill=\"#27272a\">Robertson SBT — grondsoort-zones</text>"
    ));
    for (i, z) in zones.iter().enumerate() {
        let y = top + i as f64 * row_h;
        let ty = y + 30.0;
        let num = z.number;
        let color = z.color;
        let name = xml_escape(z.name);
        b.push_str(&format!(
            "<rect x=\"24\" y=\"{y}\" width=\"40\" height=\"40\" rx=\"5\" fill=\"{color}\" stroke=\"#666\" stroke-width=\"1\"/>"
        ));
        b.push_str(&format!(
            "<text x=\"80\" y=\"{ty}\" font-family=\"Arial\" font-size=\"17\" fill=\"#27272a\"><tspan font-weight=\"bold\">Zone {num}</tspan>  —  {name}</text>"
        ));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"><rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#FFFFFF\"/>{b}</svg>"
    )
}

// ─── Metadata-overzicht (tabel per sondering) ────────────────────────

fn metadata_section(cpts: &[Cpt]) -> Section {
    let headers = vec![
        "Sondering".to_string(),
        "Project".to_string(),
        "Apparatuur".to_string(),
        "Maaiveld [m NAP]".to_string(),
        "Datum".to_string(),
        "Eindiepte [m]".to_string(),
    ];
    let rows: Vec<Vec<Value>> = cpts
        .iter()
        .map(|cpt| {
            let project = cpt.metadata.project_name.clone().unwrap_or_default();
            let equip = cpt.metadata.equipment.clone().unwrap_or_default();
            let mv = cpt
                .metadata
                .ground_level_nap
                .map(|z| format!("{:.2}", z))
                .unwrap_or_default();
            let datum = cpt
                .metadata
                .date
                .map(|d| d.format("%d-%m-%Y").to_string())
                .unwrap_or_default();
            let depth = fmt_depth(cpt_max_depth(cpt));
            vec![
                Value::from(cpt.id.clone()),
                Value::from(project),
                Value::from(equip),
                Value::from(mv),
                Value::from(datum),
                depth,
            ]
        })
        .collect();

    let table = TableBlock {
        title: Some("Metadata per sondering".to_string()),
        headers,
        rows,
        column_widths: None,
        style: TableStyle::Striped,
    };

    Section {
        title: "Metadata-overzicht".to_string(),
        level: 1,
        content: vec![
            ContentBlock::Paragraph(ParagraphBlock {
                text: "Herkomst en kalibratie-context van iedere sondering in dit rapport."
                    .to_string(),
                style: "Normal".to_string(),
            }),
            ContentBlock::Table(table),
        ],
        orientation: None,
        page_break_before: true,
    }
}

/// Minimale XML/SVG-escape voor labels (sondering-id's, grondsoort-namen).
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
