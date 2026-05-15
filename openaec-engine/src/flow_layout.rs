//! Flow layout — zone shifting on text wrap overflow and pagination.

use openaec_core::brand::BrandConfig;
use openaec_core::template_config::{ImageZone, LineZone, TextZone};
use serde_json::Value;

use crate::data_bind::resolve_bind;
use crate::font_engine::FontEngine;
use crate::pdf_backend::MM_TO_PT;
use crate::text::wrapped_text_height_mm;
use crate::zone_renderer::resolve_font_name;

/// Zones for a single page after pagination.
#[derive(Debug, Clone)]
pub struct PageZones {
    pub text_zones: Vec<TextZone>,
    pub image_zones: Vec<ImageZone>,
    pub line_zones: Vec<LineZone>,
}

/// Apply flow layout: shift text zones down when wrapping causes overflow.
///
/// Groups zones by y-position (rows). For each row, if the wrapped text
/// height exceeds the natural gap to the next row, accumulate an offset
/// and shift all zones below.
///
/// Returns the modified text zones (footer zones stay fixed).
pub fn apply_flow_layout(
    text_zones: &[TextZone],
    data: &Value,
    font_engine: &FontEngine,
    brand: &BrandConfig,
    flow_footer_y_mm: f64,
) -> Vec<TextZone> {
    // Separate content zones (above footer) and footer zones (at/below footer)
    let mut content: Vec<TextZone> = text_zones
        .iter()
        .filter(|z| z.y_mm < flow_footer_y_mm)
        .cloned()
        .collect();
    let footer: Vec<TextZone> = text_zones
        .iter()
        .filter(|z| z.y_mm >= flow_footer_y_mm)
        .cloned()
        .collect();

    // Sort content zones by y position
    content.sort_by(|a, b| a.y_mm.partial_cmp(&b.y_mm).unwrap());

    // Group zones by similar y (within 1mm tolerance = same row)
    let rows = group_by_y(&content, 1.0);

    let mut accumulated_offset = 0.0;
    let mut shifted: Vec<TextZone> = Vec::new();

    for (row_idx, row) in rows.iter().enumerate() {
        // Calculate max wrapped height for this row
        let mut max_wrapped_h = 0.0_f64;
        for zone in row {
            if zone.max_width_mm.is_some() {
                let text = resolve_bind(data, &zone.bind).unwrap_or_default();
                if !text.is_empty() {
                    let font_name = resolve_font_name(font_engine, brand, &zone.font);
                    let max_w_pt = zone.max_width_mm.unwrap() * MM_TO_PT;
                    let h = wrapped_text_height_mm(
                        &text,
                        max_w_pt,
                        &font_name,
                        zone.size,
                        zone.line_height_mm,
                        font_engine,
                    );
                    max_wrapped_h = max_wrapped_h.max(h);
                }
            }
        }

        // Natural gap: distance to next row's y
        let natural_gap = if row_idx + 1 < rows.len() {
            let next_y = rows[row_idx + 1][0].y_mm;
            let this_y = row[0].y_mm;
            next_y - this_y
        } else {
            // Last row before footer — gap to footer
            flow_footer_y_mm - row[0].y_mm
        };

        // If wrapped height exceeds natural gap, add the excess to offset
        if max_wrapped_h > natural_gap && max_wrapped_h > 0.0 {
            accumulated_offset += max_wrapped_h - natural_gap;
        }

        // Shift all zones in this row
        for zone in row {
            let mut z = (*zone).clone();
            z.y_mm += accumulated_offset;
            shifted.push(z);
        }
    }

    // Append footer zones unchanged
    shifted.extend(footer.into_iter());
    shifted
}

/// Split zones across pages when they exceed footer boundary.
pub fn paginate_flow_zones(
    text_zones: &[TextZone],
    image_zones: &[ImageZone],
    line_zones: &[LineZone],
    flow_footer_y_mm: f64,
    flow_content_start_y_mm: f64,
) -> Vec<PageZones> {
    // Separate footer zones (they repeat on every page)
    let footer_text: Vec<TextZone> = text_zones
        .iter()
        .filter(|z| z.y_mm >= flow_footer_y_mm)
        .cloned()
        .collect();
    let content_text: Vec<TextZone> = text_zones
        .iter()
        .filter(|z| z.y_mm < flow_footer_y_mm)
        .cloned()
        .collect();

    let footer_line: Vec<LineZone> = line_zones
        .iter()
        .filter(|z| z.y_mm >= flow_footer_y_mm)
        .cloned()
        .collect();
    let content_line: Vec<LineZone> = line_zones
        .iter()
        .filter(|z| z.y_mm < flow_footer_y_mm)
        .cloned()
        .collect();

    let footer_image: Vec<ImageZone> = image_zones
        .iter()
        .filter(|z| z.y_mm >= flow_footer_y_mm)
        .cloned()
        .collect();
    let content_image: Vec<ImageZone> = image_zones
        .iter()
        .filter(|z| z.y_mm < flow_footer_y_mm)
        .cloned()
        .collect();

    // If everything fits on one page, return as-is
    let max_y = content_text
        .iter()
        .map(|z| z.y_mm)
        .chain(content_image.iter().map(|z| z.y_mm + z.height_mm))
        .fold(0.0_f64, f64::max);

    if max_y < flow_footer_y_mm {
        return vec![PageZones {
            text_zones: text_zones.to_vec(),
            image_zones: image_zones.to_vec(),
            line_zones: line_zones.to_vec(),
        }];
    }

    // Split into pages
    let mut pages = Vec::new();
    let mut remaining_text = content_text;
    let mut remaining_image = content_image;
    let mut remaining_line = content_line;
    let mut page_num = 0;

    while !remaining_text.is_empty() || !remaining_image.is_empty() {
        let _y_start = if page_num == 0 { 0.0 } else { flow_content_start_y_mm };
        let _y_offset = if page_num == 0 { 0.0 } else {
            let page_range = flow_footer_y_mm - flow_content_start_y_mm;
            -(page_num as f64 * page_range - flow_content_start_y_mm)
        };

        // Collect zones that fit on this page
        let mut page_text = Vec::new();
        let mut next_text = Vec::new();
        for z in remaining_text {
            if z.y_mm < flow_footer_y_mm {
                page_text.push(z);
            } else {
                // Shift to next page
                let mut shifted = z;
                shifted.y_mm -= flow_footer_y_mm - flow_content_start_y_mm;
                next_text.push(shifted);
            }
        }

        let mut page_image = Vec::new();
        let mut next_image = Vec::new();
        for z in remaining_image {
            if z.y_mm < flow_footer_y_mm {
                page_image.push(z);
            } else {
                let mut shifted = z;
                shifted.y_mm -= flow_footer_y_mm - flow_content_start_y_mm;
                next_image.push(shifted);
            }
        }

        let page_line: Vec<LineZone> = remaining_line
            .iter()
            .filter(|z| z.y_mm < flow_footer_y_mm)
            .cloned()
            .collect();

        let next_line: Vec<LineZone> = remaining_line
            .iter()
            .filter(|z| z.y_mm >= flow_footer_y_mm)
            .map(|z| {
                let mut shifted = z.clone();
                shifted.y_mm -= flow_footer_y_mm - flow_content_start_y_mm;
                shifted
            })
            .collect();

        // Add footer zones to each page
        let mut final_text = page_text;
        final_text.extend(footer_text.clone());

        let mut final_image = page_image;
        final_image.extend(footer_image.clone());

        let mut final_line = page_line;
        final_line.extend(footer_line.clone());

        pages.push(PageZones {
            text_zones: final_text,
            image_zones: final_image,
            line_zones: final_line,
        });

        remaining_text = next_text;
        remaining_image = next_image;
        remaining_line = next_line;
        page_num += 1;
    }

    if pages.is_empty() {
        pages.push(PageZones {
            text_zones: text_zones.to_vec(),
            image_zones: image_zones.to_vec(),
            line_zones: line_zones.to_vec(),
        });
    }

    pages
}

/// Calculate how many table rows fit on one page.
pub fn rows_per_page(table_origin_y_mm: f64, max_y_mm: f64, row_height_mm: f64, has_header: bool) -> usize {
    let available = max_y_mm - table_origin_y_mm - if has_header { row_height_mm } else { 0.0 };
    (available / row_height_mm).floor().max(1.0) as usize
}

/// Paginate table data into chunks.
pub fn paginate_table_data(
    total_rows: usize,
    rows_per_page: usize,
) -> Vec<(usize, usize)> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < total_rows {
        let count = (total_rows - offset).min(rows_per_page);
        chunks.push((offset, count));
        offset += count;
    }
    if chunks.is_empty() {
        chunks.push((0, 0));
    }
    chunks
}

// ── Helpers ──

/// Group zones by y position with tolerance.
fn group_by_y(zones: &[TextZone], tolerance: f64) -> Vec<Vec<&TextZone>> {
    let mut groups: Vec<Vec<&TextZone>> = Vec::new();

    for zone in zones {
        let found = groups.iter_mut().find(|g| {
            (g[0].y_mm - zone.y_mm).abs() < tolerance
        });

        match found {
            Some(group) => group.push(zone),
            None => groups.push(vec![zone]),
        }
    }

    groups.sort_by(|a, b| a[0].y_mm.partial_cmp(&b[0].y_mm).unwrap());
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rows_per_page() {
        // available = 260 - 60 - 5.6 (header) = 194.4, /5.6 = 34
        assert_eq!(rows_per_page(60.0, 260.0, 5.6, true), 34);
        // available = 260 - 60 = 200, /5.6 = 35
        assert_eq!(rows_per_page(60.0, 260.0, 5.6, false), 35);
    }

    #[test]
    fn test_paginate_table_data() {
        let chunks = paginate_table_data(100, 35);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], (0, 35));
        assert_eq!(chunks[1], (35, 35));
        assert_eq!(chunks[2], (70, 30));
    }

    #[test]
    fn test_paginate_empty() {
        let chunks = paginate_table_data(0, 35);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, 0));
    }
}
