//! Special pages — cover, colofon, backcover, TOC.

use openaec_core::brand::BrandConfig;
use serde_json::Value;

use crate::data_bind::resolve_bind;
use crate::font_engine::FontEngine;
use crate::pdf_backend::{PageHandle, PdfBackend};
use crate::text::{x_mm_to_pt, y_mm_to_pt};
use crate::zone_renderer::resolve_color;

/// TOC entry collected during rendering.
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub page_num: usize,
    pub level: usize,
}

/// Render a cover page using brand stationery + text overlays.
///
/// Cover layout is driven by brand.pages.cover config.
/// If no specific cover config exists, renders a simple title page.
pub fn render_cover(
    backend: &mut PdfBackend,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    page: PageHandle,
    data: &Value,
    page_height_pt: f64,
) {
    let primary = resolve_color(brand, "primary");

    // Project title
    let title = resolve_bind(data, "project").unwrap_or_default();
    let title_size = brand.page_f64("cover", "title_size_ref", 28.0);
    let title_x = brand.page_f64("cover", "title_x_ref", 54.0);
    let title_y = brand.page_f64("cover", "title_y_ref", 93.0);

    if let Some(font_id) = font_engine.get_font_id(font_engine.resolve("heading")) {
        let x_pt = x_mm_to_pt(title_x) as f32;
        let y_pt = y_mm_to_pt(title_y, page_height_pt) as f32;
        backend.draw_text(page, &title, x_pt, y_pt, font_id, title_size as f32, primary);
    }

    // Subtitle / report type
    let subtitle = resolve_bind(data, "cover.subtitle")
        .or_else(|| resolve_bind(data, "report_type"))
        .unwrap_or_default();
    if !subtitle.is_empty() {
        if let Some(font_id) = font_engine.get_font_id(font_engine.resolve("body")) {
            let x_pt = x_mm_to_pt(title_x) as f32;
            let y_pt = y_mm_to_pt(title_y + 12.0, page_height_pt) as f32;
            backend.draw_text(page, &subtitle, x_pt, y_pt, font_id, 14.0, primary);
        }
    }
}

/// Render a colofon page with project metadata.
pub fn render_colofon(
    backend: &mut PdfBackend,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    page: PageHandle,
    data: &Value,
    page_height_pt: f64,
) {
    let text_color = resolve_color(brand, "text");
    let label_color = resolve_color(brand, "text_light");

    let fields = [
        ("Project:", "project"),
        ("Projectnummer:", "project_number"),
        ("Opdrachtgever:", "client"),
        ("Auteur:", "author"),
        ("Datum:", "date"),
        ("Status:", "status"),
        ("Versie:", "version"),
    ];

    let label_font = font_engine.resolve("heading").to_string();
    let value_font = font_engine.resolve("body").to_string();

    let label_id = font_engine.get_font_id(&label_font);
    let value_id = font_engine.get_font_id(&value_font);

    let x_label = x_mm_to_pt(25.0) as f32;
    let x_value = x_mm_to_pt(70.0) as f32;
    let mut y_mm = 80.0;

    for (label, bind) in &fields {
        let value = resolve_bind(data, bind).unwrap_or_default();
        if value.is_empty() {
            continue;
        }

        let y_pt = y_mm_to_pt(y_mm, page_height_pt) as f32;

        if let Some(fid) = label_id {
            backend.draw_text(page, label, x_label, y_pt, fid, 9.0, label_color);
        }
        if let Some(fid) = value_id {
            backend.draw_text(page, &value, x_value, y_pt, fid, 9.0, text_color);
        }

        y_mm += 8.0;
    }
}

/// Render a backcover page (typically just brand stationery).
pub fn render_backcover(
    _backend: &mut PdfBackend,
    _font_engine: &FontEngine,
    _brand: &BrandConfig,
    _page: PageHandle,
    _page_height_pt: f64,
) {
    // Backcover is usually just stationery — no additional rendering needed.
    // Stationery is already embedded by the engine.
}

/// Render a table of contents page.
pub fn render_toc(
    backend: &mut PdfBackend,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    page: PageHandle,
    entries: &[TocEntry],
    page_height_pt: f64,
    page_width_mm: f64,
) {
    let primary = resolve_color(brand, "primary");
    let text_color = resolve_color(brand, "text");

    // Title
    let heading_font = font_engine.resolve("heading").to_string();
    if let Some(fid) = font_engine.get_font_id(&heading_font) {
        let y_pt = y_mm_to_pt(35.0, page_height_pt) as f32;
        backend.draw_text(page, "Inhoudsopgave", x_mm_to_pt(25.0) as f32, y_pt, fid, 18.0, primary);
    }

    // Entries
    let body_font = font_engine.resolve("body").to_string();
    let body_id = font_engine.get_font_id(&body_font);
    let heading_id = font_engine.get_font_id(&heading_font);

    let mut y_mm = 50.0;
    for entry in entries {
        let indent = entry.level as f64 * 8.0;
        let x_mm = 25.0 + indent;
        let size = if entry.level == 0 { 11.0 } else { 9.5 };
        let color = if entry.level == 0 { primary } else { text_color };
        let fid = if entry.level == 0 { heading_id } else { body_id };

        if let Some(fid) = fid {
            let y_pt = y_mm_to_pt(y_mm, page_height_pt) as f32;
            backend.draw_text(page, &entry.title, x_mm_to_pt(x_mm) as f32, y_pt, fid, size as f32, color);

            // Page number right-aligned
            let page_str = entry.page_num.to_string();
            let num_x = x_mm_to_pt(page_width_mm - 20.0) as f32
                - font_engine.measure_text(&page_str, &body_font, size) as f32;
            let num_fid = body_id.unwrap_or(fid);
            backend.draw_text(page, &page_str, num_x, y_pt, num_fid, size as f32, text_color);
        }

        y_mm += if entry.level == 0 { 7.0 } else { 5.5 };
    }
}
