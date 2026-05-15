//! Zone renderers — draw text/image/line/table zones onto PDF pages.

use std::path::Path;

use openaec_core::brand::BrandConfig;
use openaec_core::template_config::{
    ImageZone, LineZone, TableConfig, TextAlign, TextZone,
};
use serde_json::Value;

use crate::data_bind::{format_value, resolve_bind, resolve_bind_value};
use crate::font_engine::FontEngine;
use crate::pdf_backend::{Color, ImageFormat, MM_TO_PT, PageHandle, PdfBackend};
use crate::text::{self, wrap_text, x_mm_to_pt};

/// Render all text zones for a page.
pub fn draw_text_zones(
    backend: &mut PdfBackend,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    page: PageHandle,
    zones: &[TextZone],
    data: &Value,
    page_height_pt: f64,
    abs_page_num: usize,
) {
    for zone in zones {
        // Resolve data value
        let text_value = if zone.bind == "_page_number" {
            Some(abs_page_num.to_string())
        } else {
            resolve_bind(data, &zone.bind)
        };

        let text_value = match text_value {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };

        // Resolve font and color from brand
        let font_name = resolve_font_name(font_engine, brand, &zone.font);
        let color = resolve_color(brand, &zone.color);
        let font_id = match font_engine.get_font_id(&font_name) {
            Some(id) => id,
            None => continue,
        };

        let x_pt = x_mm_to_pt(zone.x_mm) as f32;
        let size = zone.size;

        // Wrap text if max_width_mm is set
        let lines = if let Some(max_w) = zone.max_width_mm {
            let max_w_pt = max_w * MM_TO_PT;
            wrap_text(&text_value, max_w_pt, &font_name, size, font_engine)
        } else {
            vec![text_value]
        };

        let _line_height_pt = (zone.line_height_mm * MM_TO_PT) as f32;

        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }

            let y_mm = zone.y_mm + (i as f64 * zone.line_height_mm);
            let y_pt = text::text_y_mm_to_pt(y_mm, page_height_pt, &font_name, size, font_engine);

            // Handle alignment
            let final_x = match zone.align {
                TextAlign::Right => {
                    let w = font_engine.measure_text(line, &font_name, size);
                    x_pt - w as f32
                }
                TextAlign::Center => {
                    let w = font_engine.measure_text(line, &font_name, size);
                    x_pt - (w as f32 / 2.0)
                }
                TextAlign::Left => x_pt,
            };

            backend.draw_text(page, line, final_x, y_pt as f32, font_id, size as f32, color);
        }
    }
}

/// Render all line zones for a page.
pub fn draw_line_zones(
    backend: &mut PdfBackend,
    brand: &BrandConfig,
    page: PageHandle,
    zones: &[LineZone],
    page_height_pt: f64,
) {
    for zone in zones {
        let color = resolve_color(brand, &zone.color);
        let x0 = x_mm_to_pt(zone.x0_mm) as f32;
        let x1 = x_mm_to_pt(zone.x1_mm) as f32;
        let y = text::y_mm_to_pt(zone.y_mm, page_height_pt) as f32;

        backend.draw_line(page, x0, y, x1, y, zone.width_pt as f32, color);
    }
}

/// Render all image zones for a page.
pub fn draw_image_zones(
    backend: &mut PdfBackend,
    page: PageHandle,
    zones: &[ImageZone],
    data: &Value,
    page_height_pt: f64,
    base_dir: &Path,
) {
    for zone in zones {
        // Resolve image path from data
        let img_path_str = resolve_bind(data, &zone.bind)
            .or_else(|| {
                if !zone.fallback.is_empty() {
                    Some(zone.fallback.clone())
                } else {
                    None
                }
            });

        let img_path_str = match img_path_str {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };

        // Load image
        let img_path = base_dir.join(&img_path_str);
        let img_data = match std::fs::read(&img_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Detect format
        let format = if img_path_str.ends_with(".jpg") || img_path_str.ends_with(".jpeg") {
            ImageFormat::Jpeg
        } else {
            ImageFormat::Png
        };

        // Get dimensions
        let (iw, ih) = match image::image_dimensions(&img_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let img_idx = backend.register_image(img_data, format, iw, ih);

        let x_pt = x_mm_to_pt(zone.x_mm) as f32;
        let w_pt = (zone.width_mm * MM_TO_PT) as f32;
        let h_pt = (zone.height_mm * MM_TO_PT) as f32;
        // y_mm is top of image box → PDF y is bottom of image box
        let y_pt = (text::y_mm_to_pt(zone.y_mm, page_height_pt) - h_pt as f64) as f32;

        backend.draw_image(page, img_idx, x_pt, y_pt, w_pt, h_pt);
    }
}

/// Render a table on a page.
pub fn draw_table(
    backend: &mut PdfBackend,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    page: PageHandle,
    table: &TableConfig,
    data: &Value,
    page_height_pt: f64,
    row_offset: usize,
    max_rows: Option<usize>,
) {
    // Resolve data array
    let rows = match resolve_bind_value(data, &table.data_bind) {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };

    let end = match max_rows {
        Some(max) => (row_offset + max).min(rows.len()),
        None => rows.len(),
    };
    let visible_rows = &rows[row_offset..end];

    let origin_x_pt = x_mm_to_pt(table.origin_x_mm) as f32;
    let row_height_pt = (table.row_height_mm * MM_TO_PT) as f32;

    let mut current_y_mm = table.origin_y_mm;

    // Draw header
    if table.show_header {
        let header_font_name = resolve_font_name(font_engine, brand, &table.header_font);
        let header_color = resolve_color(brand, &table.header_color);
        let header_font_id = font_engine.get_font_id(&header_font_name);

        // Header background
        if let Some(ref bg) = table.header_bg {
            let bg_color = resolve_color(brand, bg);
            let total_w: f64 = table.columns.iter().map(|c| c.width_mm).sum();
            let x = origin_x_pt;
            let y = text::y_mm_to_pt(current_y_mm, page_height_pt) as f32;
            backend.draw_rect(page, x, y - row_height_pt, (total_w * MM_TO_PT) as f32, row_height_pt, Some(bg_color), None);
        }

        if let Some(font_id) = header_font_id {
            let mut col_x_mm = table.origin_x_mm;
            for col in &table.columns {
                let label = col.header.as_deref().unwrap_or(&col.field);
                let y_pt = text::text_y_mm_to_pt(current_y_mm, page_height_pt, &header_font_name, table.header_size, font_engine) as f32;
                let x_pt = x_mm_to_pt(col_x_mm) as f32;
                backend.draw_text(page, label, x_pt, y_pt, font_id, table.header_size as f32, header_color);
                col_x_mm += col.width_mm;
            }
        }
        current_y_mm += table.row_height_mm;
    }

    // Draw rows
    for (ri, row) in visible_rows.iter().enumerate() {
        // Alternating row background
        if ri % 2 == 1 {
            if let Some(ref alt_bg) = table.alt_row_bg {
                let bg_color = resolve_color(brand, alt_bg);
                let total_w: f64 = table.columns.iter().map(|c| c.width_mm).sum();
                let x = origin_x_pt;
                let y = text::y_mm_to_pt(current_y_mm, page_height_pt) as f32;
                backend.draw_rect(page, x, y - row_height_pt, (total_w * MM_TO_PT) as f32, row_height_pt, Some(bg_color), None);
            }
        }

        let mut col_x_mm = table.origin_x_mm;
        for col in &table.columns {
            let cell_value = resolve_bind(row, &col.field).unwrap_or_default();
            let formatted = format_value(&cell_value, col.format.as_deref());

            let col_font = col.body_font_or(&table, brand);
            let col_size = col.body_size_or(&table);
            let col_color = col.body_color_or(&table, brand);

            let font_name = resolve_font_name(font_engine, brand, &col_font);
            if let Some(font_id) = font_engine.get_font_id(&font_name) {
                let y_pt = text::text_y_mm_to_pt(current_y_mm, page_height_pt, &font_name, col_size, font_engine) as f32;

                let x_pt = match col.align {
                    TextAlign::Right => {
                        let w = font_engine.measure_text(&formatted, &font_name, col_size);
                        x_mm_to_pt(col_x_mm + col.width_mm) as f32 - w as f32
                    }
                    TextAlign::Center => {
                        let w = font_engine.measure_text(&formatted, &font_name, col_size);
                        x_mm_to_pt(col_x_mm + col.width_mm / 2.0) as f32 - w as f32 / 2.0
                    }
                    TextAlign::Left => x_mm_to_pt(col_x_mm) as f32,
                };

                backend.draw_text(page, &formatted, x_pt, y_pt, font_id, col_size as f32, col_color);
            }
            col_x_mm += col.width_mm;
        }
        current_y_mm += table.row_height_mm;
    }
}

// ── Helpers ──

/// Resolve a brand color name to a Color. Falls back to black.
pub fn resolve_color(brand: &BrandConfig, name: &str) -> Color {
    if name.starts_with('#') {
        return Color::from_hex(name);
    }
    brand
        .colors
        .get(name)
        .map(|hex| Color::from_hex(hex))
        .unwrap_or(Color::black())
}

/// Resolve a brand font name to a loaded font name.
pub fn resolve_font_name(font_engine: &FontEngine, brand: &BrandConfig, logical: &str) -> String {
    // Check brand fonts mapping first (heading → GothamBold, etc.)
    if let Some(real_name) = brand.fonts.get(logical) {
        if font_engine.has_font(real_name) {
            return real_name.clone();
        }
    }
    // Try the name directly
    if font_engine.has_font(logical) {
        return logical.to_string();
    }
    // Use font engine fallback
    font_engine.resolve(logical).to_string()
}

/// Helper trait for TableColumn to resolve body styling with fallbacks.
trait TableColumnExt {
    fn body_font_or(&self, table: &TableConfig, brand: &BrandConfig) -> String;
    fn body_size_or(&self, table: &TableConfig) -> f64;
    fn body_color_or(&self, table: &TableConfig, brand: &BrandConfig) -> Color;
}

impl TableColumnExt for openaec_core::template_config::TableColumn {
    fn body_font_or(&self, table: &TableConfig, _brand: &BrandConfig) -> String {
        table.body_font.clone().unwrap_or_else(|| self.font.clone())
    }

    fn body_size_or(&self, table: &TableConfig) -> f64 {
        table.body_size.unwrap_or(self.size)
    }

    fn body_color_or(&self, table: &TableConfig, brand: &BrandConfig) -> Color {
        let color_name = table.body_color.as_deref().unwrap_or(&self.color);
        resolve_color(brand, color_name)
    }
}
