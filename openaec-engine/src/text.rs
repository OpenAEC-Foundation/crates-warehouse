//! Text wrapping and coordinate conversion utilities.

use crate::font_engine::FontEngine;
use crate::pdf_backend::MM_TO_PT;

/// Convert YAML y_mm (top-down, from top of page) to PDF y_pt (bottom-up).
pub fn y_mm_to_pt(y_mm: f64, page_height_pt: f64) -> f64 {
    page_height_pt - (y_mm * MM_TO_PT)
}

/// Convert x_mm to x_pt.
pub fn x_mm_to_pt(x_mm: f64) -> f64 {
    x_mm * MM_TO_PT
}

/// Convert y_mm to PDF y_pt with font ascent correction.
/// Text in PDF is positioned at the baseline; YAML y_mm is the top of the text bbox.
pub fn text_y_mm_to_pt(
    y_mm: f64,
    page_height_pt: f64,
    font_name: &str,
    size: f64,
    font_engine: &FontEngine,
) -> f64 {
    let y_pt = y_mm_to_pt(y_mm, page_height_pt);
    let ascent = font_engine.ascent(font_name, size);
    y_pt - ascent
}

/// Word-wrap text to fit within max_width_pt.
///
/// Returns a Vec of lines. Simple word-boundary splitting.
pub fn wrap_text(
    text: &str,
    max_width_pt: f64,
    font_name: &str,
    size: f64,
    font_engine: &FontEngine,
) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();

    for input_line in text.split('\n') {
        let words: Vec<&str> = input_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in &words {
            if current_line.is_empty() {
                // First word — always add it even if wider than max
                current_line = word.to_string();
                continue;
            }

            let test = format!("{} {}", current_line, word);
            let test_width = font_engine.measure_text(&test, font_name, size);

            if test_width <= max_width_pt {
                current_line = test;
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        lines.push(current_line);
    }

    lines
}

/// Calculate wrapped text height in mm.
pub fn wrapped_text_height_mm(
    text: &str,
    max_width_pt: f64,
    font_name: &str,
    size: f64,
    line_height_mm: f64,
    font_engine: &FontEngine,
) -> f64 {
    let lines = wrap_text(text, max_width_pt, font_name, size, font_engine);
    let num_lines = lines.len().max(1);
    num_lines as f64 * line_height_mm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_y_mm_to_pt() {
        let page_h = 297.0 * MM_TO_PT; // A4 height in pt
        let y = y_mm_to_pt(0.0, page_h);
        assert!((y - page_h).abs() < 0.01); // top of page

        let y_bottom = y_mm_to_pt(297.0, page_h);
        assert!(y_bottom.abs() < 0.01); // bottom of page
    }

    #[test]
    fn test_x_mm_to_pt() {
        let x = x_mm_to_pt(25.4); // 1 inch
        assert!((x - 72.0).abs() < 0.01);
    }

    #[test]
    fn test_wrap_empty() {
        let engine = FontEngine::new();
        let lines = wrap_text("", 100.0, "test", 10.0, &engine);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }

    #[test]
    fn test_wrap_single_word() {
        let engine = FontEngine::new();
        let lines = wrap_text("Hello", 1000.0, "test", 10.0, &engine);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Hello");
    }

    #[test]
    fn test_wrap_preserves_newlines() {
        let engine = FontEngine::new();
        let lines = wrap_text("Line 1\nLine 2", 1000.0, "test", 10.0, &engine);
        assert_eq!(lines.len(), 2);
    }
}
