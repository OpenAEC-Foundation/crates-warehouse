//! Engine — generates PDF from ReportData.
//!
//! Connects the data layer (schema, brand, tenant) to the layout engine.
//! This is the main entry point for report generation.

use std::path::{Path, PathBuf};

use openaec_layout::{
    shared_font_registry, A4, DocTemplate, DrawList, Flowable, Frame, Mm, PageCallback,
    PageTemplate, Paragraph, ParagraphStyle, Pt, Rect, SharedFontRegistry, Size, Spacer,
};

use crate::block_renderer::render_section;
use crate::brand::{BrandConfig, BrandError, BrandIdentity, BrandLoader, ZoneConfig};
use crate::schema::ReportData;
use crate::special_pages;
use crate::tenant::TenantConfig;

// ── Error type ──────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Layout error: {0}")]
    Layout(#[from] openaec_layout::LayoutError),

    #[error("Brand error: {0}")]
    Brand(#[from] BrandError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ── Public API ──────────────────────────────────────────────────────────

/// Generate a PDF report from report data and write to a file.
pub fn generate_pdf(data: &ReportData, output: &Path) -> Result<Vec<u8>, EngineError> {
    let pdf_bytes = generate_pdf_bytes(data)?;
    std::fs::write(output, &pdf_bytes)?;
    Ok(pdf_bytes)
}

/// Generate a PDF report from report data and return the bytes.
pub fn generate_pdf_bytes(data: &ReportData) -> Result<Vec<u8>, EngineError> {
    // 1. Load tenant config
    let tenant = TenantConfig::new(None, None);

    // 2. Load brand (graceful fallback to empty brand)
    let loader = BrandLoader::new(tenant.clone());
    let brand = loader.load_default().unwrap_or_else(|_| default_brand());

    // 3. Set up fonts
    let fonts = shared_font_registry();
    setup_fonts(&fonts, &tenant);

    // 4. Create DocTemplate with content frame from brand
    let mut doc = DocTemplate::new(&data.project, fonts);
    let frame = build_content_frame(&brand);
    let callback = build_footer_callback(&brand);
    let mut template = PageTemplate::new("content", A4, frame);
    if let Some(cb) = callback {
        template = template.with_callback(cb);
    }
    doc.add_page_template(template);

    // 5. Add special pages
    add_special_pages(&mut doc, data, &brand);

    // 6. Build flowables from report data
    let flowables = build_flowables(data, &brand);

    // 7. Build PDF
    doc.build_to_bytes(flowables).map_err(EngineError::Layout)
}

/// Generate a PDF with explicit tenant and brand configuration.
pub fn generate_pdf_with_config(
    data: &ReportData,
    tenant: &TenantConfig,
    brand: &BrandConfig,
    output: &Path,
) -> Result<Vec<u8>, EngineError> {
    let fonts = shared_font_registry();
    setup_fonts(&fonts, tenant);

    let mut doc = DocTemplate::new(&data.project, fonts);
    let frame = build_content_frame(brand);
    let callback = build_footer_callback(brand);
    let mut template = PageTemplate::new("content", A4, frame);
    if let Some(cb) = callback {
        template = template.with_callback(cb);
    }
    doc.add_page_template(template);

    // Add special pages
    add_special_pages(&mut doc, data, brand);

    let flowables = build_flowables(data, brand);
    let pdf_bytes = doc.build_to_bytes(flowables).map_err(EngineError::Layout)?;
    std::fs::write(output, &pdf_bytes)?;
    Ok(pdf_bytes)
}

// ── Special page integration ────────────────────────────────────────────

/// Add special pages (cover, colofon, TOC, backcover) to the document.
fn add_special_pages(doc: &mut DocTemplate, data: &ReportData, brand: &BrandConfig) {
    // Cover page
    if data.cover.is_some() {
        doc.add_pre_page(special_pages::build_cover_page(data, brand));
    }

    // Colofon page (if enabled)
    if data.colofon.as_ref().is_some_and(|c| c.enabled) {
        doc.add_pre_page(special_pages::build_colofon_page(data, brand));
    }

    // TOC page (if enabled)
    if data.toc.as_ref().is_some_and(|t| t.enabled) {
        doc.add_pre_page(special_pages::build_toc_page(data, brand));
    }

    // Backcover page (if enabled)
    if data.backcover.as_ref().is_some_and(|b| b.enabled) {
        doc.add_post_page(special_pages::build_backcover_page(data, brand));
    }
}

// ── Content frame from brand ────────────────────────────────────────────

/// Build a content Frame from `brand.stationery.content.content_frame`.
///
/// Falls back to 20mm margins if no content_frame is configured.
fn build_content_frame(brand: &BrandConfig) -> Frame {
    if let Some(content_stationery) = brand.stationery.get("content")
        && let Some(ref cf) = content_stationery.content_frame
    {
        Frame::new(Rect::new(
            Pt(cf.x_pt as f32),
            Pt(cf.y_pt as f32),
            Pt(cf.width_pt as f32),
            Pt(cf.height_pt as f32),
        ))
    } else {
        // Fallback: 20mm margins
        let margin = Pt::from(Mm(20.0));
        Frame::new(Rect::new(
            margin,
            margin,
            Pt(A4.width.0 - margin.0 * 2.0),
            Pt(A4.height.0 - margin.0 * 2.0),
        ))
    }
}

/// Build a footer PageCallback from `brand.footer` config.
///
/// Returns None if footer height is 0 or no elements are defined.
fn build_footer_callback(brand: &BrandConfig) -> Option<Box<dyn PageCallback>> {
    if brand.footer.height == 0.0 || brand.footer.elements.is_empty() {
        return None;
    }

    let elements: Vec<FooterElement> = brand
        .footer
        .elements
        .iter()
        .filter(|el| el.element_type == "text")
        .map(|el| {
            // Resolve font reference
            let font_name = el
                .font
                .as_deref()
                .map(|f| brand.resolve_font(f).to_string())
                .unwrap_or_else(|| brand.resolve_font_name("body"));

            // Resolve color reference
            let color = el
                .color
                .as_deref()
                .and_then(|c| {
                    brand
                        .resolve_color(c)
                        .and_then(openaec_layout::Color::from_hex)
                })
                .unwrap_or(openaec_layout::Color::rgb(69, 36, 61));

            FooterElement {
                content: el.content.clone().unwrap_or_default(),
                x_mm: el.x,
                y_mm: el.y,
                font_name,
                font_size: el.size.unwrap_or(9.5) as f32,
                color,
                align: el.align.clone().unwrap_or_else(|| "left".to_string()),
            }
        })
        .collect();

    if elements.is_empty() {
        return None;
    }

    Some(Box::new(BrandFooterCallback { elements }))
}

/// A single footer element resolved from brand config.
#[derive(Debug, Clone)]
struct FooterElement {
    content: String,
    x_mm: f64,
    y_mm: f64,
    font_name: String,
    font_size: f32,
    color: openaec_layout::Color,
    align: String,
}

/// Footer callback that renders brand footer elements on each content page.
#[derive(Debug, Clone)]
struct BrandFooterCallback {
    elements: Vec<FooterElement>,
}

impl PageCallback for BrandFooterCallback {
    fn on_page(
        &self,
        draw_list: &mut DrawList,
        page_num: usize,
        total_pages: usize,
        page_size: Size,
    ) {
        // Only render on pass 2 (when total_pages is known)
        if total_pages == 0 {
            return;
        }

        let page_h = page_size.height;

        for el in &self.elements {
            // Variable substitution
            let text = el
                .content
                .replace("{page}", &page_num.to_string())
                .replace("{total_pages}", &total_pages.to_string());

            // Convert mm to points (footer y is from bottom of page)
            let x = Pt::from(Mm(el.x_mm as f32));
            let y = Pt(page_h.0 - Pt::from(Mm(el.y_mm as f32)).0);

            draw_list.set_fill_color(el.color);
            draw_list.set_font(&el.font_name, Pt(el.font_size));

            match el.align.as_str() {
                "right" => draw_list.draw_text_right(x, y, &text),
                "center" => draw_list.draw_text_center(x, y, &text),
                _ => draw_list.draw_text(x, y, &text),
            }
        }
    }
}

// ── Flowable construction ───────────────────────────────────────────────

/// Build the full list of flowables from report data.
fn build_flowables(data: &ReportData, brand: &BrandConfig) -> Vec<Box<dyn Flowable>> {
    let mut flowables: Vec<Box<dyn Flowable>> = Vec::new();

    // Title
    let text_color = brand
        .resolve_color("$text")
        .and_then(openaec_layout::Color::from_hex)
        .unwrap_or(openaec_layout::Color::rgb(69, 36, 61));

    let title_style = ParagraphStyle {
        font_size: Pt(18.0),
        leading: Pt(24.0),
        bold: true,
        text_color,
        space_after: Pt(6.0),
        ..Default::default()
    };
    flowables.push(Box::new(Paragraph::new(&data.project, title_style)));

    // Subtitle (from cover if available)
    if let Some(ref cover) = data.cover
        && let Some(ref subtitle) = cover.subtitle {
            let subtitle_style = ParagraphStyle {
                font_size: Pt(12.0),
                leading: Pt(16.0),
                text_color,
                space_after: Pt(4.0),
                ..Default::default()
            };
            flowables.push(Box::new(Paragraph::new(subtitle, subtitle_style)));
        }

    // Metadata line
    let meta_parts: Vec<String> = [
        data.project_number
            .as_ref()
            .map(|n| format!("Project: {}", n)),
        data.client.as_ref().map(|c| format!("Opdrachtgever: {}", c)),
        data.date.as_ref().map(|d| format!("Datum: {}", d)),
    ]
    .into_iter()
    .flatten()
    .collect();

    if !meta_parts.is_empty() {
        let meta_style = ParagraphStyle {
            font_size: Pt(9.0),
            leading: Pt(12.0),
            text_color: openaec_layout::Color::GREY,
            space_after: Pt(4.0),
            ..Default::default()
        };
        flowables.push(Box::new(Paragraph::new(
            meta_parts.join("  |  "),
            meta_style,
        )));
    }

    flowables.push(Box::new(Spacer::from_mm(8.0)));

    // Sections
    for (i, section) in data.sections.iter().enumerate() {
        let section_flowables = render_section(section, brand, i + 1);
        flowables.extend(section_flowables);
    }

    flowables
}

// ── Font setup ──────────────────────────────────────────────────────────

/// Register fonts from tenant directory and monorepo fallback paths.
fn setup_fonts(fonts: &SharedFontRegistry, tenant: &TenantConfig) {
    let mut font_dirs: Vec<PathBuf> = Vec::new();

    // Tenant fonts directory
    if let Some(dir) = tenant.fonts_dir() {
        font_dirs.push(dir.to_path_buf());
    }

    // OPENAEC_FONTS_DIR env var (Docker / production)
    if let Ok(fonts_env) = std::env::var("OPENAEC_FONTS_DIR") {
        let dir = PathBuf::from(fonts_env);
        if dir.is_dir() {
            font_dirs.push(dir);
        }
    }

    // Monorepo Python package fonts (fallback for dev)
    let candidates = [
        PathBuf::from("../src/openaec_reports/assets/fonts"),     // CWD = rust/
        PathBuf::from("src/openaec_reports/assets/fonts"),         // CWD = monorepo root
        PathBuf::from("../../src/openaec_reports/assets/fonts"),   // CWD = rust/crates/X
    ];
    for candidate in &candidates {
        if candidate.is_dir() {
            font_dirs.push(candidate.clone());
            break;
        }
    }

    // Scan directories for TTF files and register them
    let mut registry = fonts.lock().unwrap();
    let mut registered_names: Vec<String> = Vec::new();

    for dir in &font_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension()
                    && (ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf"))
                    && let Ok(data) = std::fs::read(&path)
                {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    tracing::info!(font = %name, path = %path.display(), "Registering font");
                    if let Err(e) = registry.register_ttf_bytes(&name, data) {
                        tracing::warn!(font = %name, error = %e, "Failed to register font");
                    } else {
                        registered_names.push(name);
                    }
                }
            }
        }
    }

    // Register base-name aliases: "LiberationSans-Regular" → "LiberationSans",
    // "Gotham-Book" → "Gotham", etc. This ensures ParagraphStyle default
    // font_name "LiberationSans" resolves correctly.
    for name in &registered_names {
        let suffixes = ["-Regular", "-Book", "-Roman", "-Normal"];
        for suffix in &suffixes {
            if let Some(base) = name.strip_suffix(suffix)
                && registry.get(base).is_none()
            {
                registry.register_alias(base, name);
                tracing::info!(alias = base, target = %name, "Registered font alias");
            }
        }
    }

    // Register CamelCase aliases: "Gotham-Bold" → "GothamBold",
    // "Gotham-BookItalic" → "GothamBookItalic", etc.
    // Brand.yaml uses CamelCase names, but font files use hyphens.
    for name in &registered_names {
        let no_hyphen = name.replace('-', "");
        if no_hyphen != *name && registry.get(&no_hyphen).is_none() {
            registry.register_alias(&no_hyphen, name);
            tracing::info!(alias = %no_hyphen, target = %name, "Registered CamelCase alias");
        }
    }

}

// ── Defaults ────────────────────────────────────────────────────────────

/// Create a minimal default BrandConfig when no brand file is available.
fn default_brand() -> BrandConfig {
    let mut colors = std::collections::HashMap::new();
    colors.insert("primary".to_string(), "#40124A".to_string());
    colors.insert("secondary".to_string(), "#38BDA0".to_string());
    colors.insert("text".to_string(), "#45243D".to_string());

    BrandConfig {
        brand: BrandIdentity {
            name: "Default".to_string(),
            slug: "default".to_string(),
            tenant: None,
            tagline: None,
        },
        colors,
        fonts: std::collections::HashMap::new(),
        logos: std::collections::HashMap::new(),
        contact: std::collections::HashMap::new(),
        header: ZoneConfig::default(),
        footer: ZoneConfig::default(),
        styles: std::collections::HashMap::new(),
        pages: std::collections::HashMap::new(),
        stationery: std::collections::HashMap::new(),
        modules: std::collections::HashMap::new(),
        module_config: std::collections::HashMap::new(),
        tenant_modules: serde_yaml::Value::Null,
        font_files: std::collections::HashMap::new(),
        brand_dir: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    #[test]
    fn test_build_flowables_minimal() {
        let data = ReportData {
            template: "structural".to_string(),
            project: "Test Project".to_string(),
            tenant: None,
            format: PaperFormat::A4,
            orientation: Orientation::Portrait,
            project_number: None,
            client: None,
            author: "Test".to_string(),
            date: None,
            version: "1.0".to_string(),
            status: ReportStatus::Concept,
            cover: None,
            colofon: None,
            toc: None,
            sections: vec![],
            backcover: None,
            metadata: std::collections::HashMap::new(),
        };

        let brand = default_brand();
        let flowables = build_flowables(&data, &brand);

        // At minimum: title + spacer
        assert!(flowables.len() >= 2);
    }

    #[test]
    fn test_build_flowables_with_sections() {
        let data = ReportData {
            template: "structural".to_string(),
            project: "Test Project".to_string(),
            tenant: None,
            format: PaperFormat::A4,
            orientation: Orientation::Portrait,
            project_number: Some("2026-001".to_string()),
            client: Some("Test Client".to_string()),
            author: "Test".to_string(),
            date: Some("2026-03-15".to_string()),
            version: "1.0".to_string(),
            status: ReportStatus::Concept,
            cover: None,
            colofon: None,
            toc: None,
            sections: vec![Section {
                title: "Uitgangspunten".to_string(),
                level: 1,
                content: vec![ContentBlock::Paragraph(ParagraphBlock {
                    text: "Dit is een test.".to_string(),
                    style: "Normal".to_string(),
                })],
                orientation: None,
                page_break_before: false,
            }],
            backcover: None,
            metadata: std::collections::HashMap::new(),
        };

        let brand = default_brand();
        let flowables = build_flowables(&data, &brand);

        // title + meta + spacer + section heading + paragraph + section spacer
        assert!(flowables.len() >= 5);
    }

    #[test]
    fn test_default_brand() {
        let brand = default_brand();
        assert_eq!(brand.resolve_color("$primary"), Some("#40124A"));
        assert_eq!(brand.resolve_color("$text"), Some("#45243D"));
    }
}
