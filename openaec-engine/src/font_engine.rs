//! Font loading, measurement, and management.
//!
//! Loads TTF fonts, measures text width (critical for wrapping),
//! and provides font ascent/descent for baseline correction.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::error::{EngineError, Result};
use crate::pdf_backend::{FontId, PdfBackend};

/// A loaded font with parsed metrics.
pub struct LoadedFont {
    pub data: Arc<Vec<u8>>,
    pub pdf_id: FontId,
    units_per_em: f64,
    ascender: f64,
    descender: f64,
    glyph_widths: Vec<u16>, // indexed by glyph ID
}

/// Font engine — manages loaded fonts and provides text measurement.
pub struct FontEngine {
    fonts: HashMap<String, LoadedFont>,
    fallback_chain: Vec<(String, String)>, // (logical name, resolved name)
}

impl FontEngine {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
            fallback_chain: Vec::new(),
        }
    }

    /// Load a font from raw TTF/OTF bytes and register it with the PDF backend.
    pub fn load_font(
        &mut self,
        name: &str,
        data: Vec<u8>,
        backend: &mut PdfBackend,
    ) -> Result<FontId> {
        let face = ttf_parser::Face::parse(&data, 0)
            .map_err(|e| EngineError::Font(format!("Parse '{}': {}", name, e)))?;

        let units_per_em = face.units_per_em() as f64;
        let ascender = face.ascender() as f64;
        let descender = face.descender() as f64;

        // Collect glyph widths
        let num_glyphs = face.number_of_glyphs();
        let mut glyph_widths = Vec::with_capacity(num_glyphs as usize);
        for gid in 0..num_glyphs {
            let w = face
                .glyph_hor_advance(ttf_parser::GlyphId(gid))
                .unwrap_or(0);
            glyph_widths.push(w);
        }

        let data = Arc::new(data);
        let pdf_id = backend.register_font(name.to_string(), data.as_ref().clone());

        self.fonts.insert(name.to_string(), LoadedFont {
            data,
            pdf_id,
            units_per_em,
            ascender,
            descender,
            glyph_widths,
        });

        Ok(pdf_id)
    }

    /// Load a font from a file path.
    pub fn load_font_file(
        &mut self,
        name: &str,
        path: &Path,
        backend: &mut PdfBackend,
    ) -> Result<FontId> {
        let data = std::fs::read(path)?;
        self.load_font(name, data, backend)
    }

    /// Measure text width in points at given font size.
    pub fn measure_text(&self, text: &str, font_name: &str, size: f64) -> f64 {
        let font = match self.fonts.get(font_name) {
            Some(f) => f,
            None => return text.len() as f64 * size * 0.5, // rough fallback
        };

        let face = match ttf_parser::Face::parse(&font.data, 0) {
            Ok(f) => f,
            Err(_) => return text.len() as f64 * size * 0.5,
        };

        let scale = size / font.units_per_em;
        let mut width = 0.0;
        for ch in text.chars() {
            let gid = face.glyph_index(ch).map(|g| g.0 as usize).unwrap_or(0);
            let advance = if gid < font.glyph_widths.len() {
                font.glyph_widths[gid] as f64
            } else {
                0.0
            };
            width += advance * scale;
        }
        width
    }

    /// Font ascent in points at given size.
    pub fn ascent(&self, font_name: &str, size: f64) -> f64 {
        self.fonts.get(font_name).map_or(size * 0.8, |f| {
            f.ascender / f.units_per_em * size
        })
    }

    /// Font descent in points at given size (negative value).
    pub fn descent(&self, font_name: &str, size: f64) -> f64 {
        self.fonts.get(font_name).map_or(size * -0.2, |f| {
            f.descender / f.units_per_em * size
        })
    }

    /// Get the FontId for a font name, with fallback.
    pub fn get_font_id(&self, name: &str) -> Option<FontId> {
        self.fonts.get(name).map(|f| f.pdf_id)
    }

    /// Resolve a logical font name (e.g. "heading", "body") to a loaded font name.
    pub fn resolve<'a>(&'a self, logical_name: &'a str) -> &'a str {
        // If the name itself is loaded, use it directly
        if self.fonts.contains_key(logical_name) {
            return logical_name;
        }
        // Check fallback chain
        for (from, to) in &self.fallback_chain {
            if from == logical_name {
                return to;
            }
        }
        // Last resort: return first loaded font
        self.fonts.keys().next().map(|s| s.as_str()).unwrap_or("Helvetica")
    }

    /// Register a fallback mapping: logical name → loaded font name.
    pub fn add_fallback(&mut self, logical_name: &str, resolved_name: &str) {
        self.fallback_chain.push((logical_name.to_string(), resolved_name.to_string()));
    }

    /// Check if a font is loaded.
    pub fn has_font(&self, name: &str) -> bool {
        self.fonts.contains_key(name)
    }

    /// Discover and load fonts from a directory.
    pub fn load_fonts_from_dir(
        &mut self,
        dir: &Path,
        backend: &mut PdfBackend,
    ) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        if !dir.exists() {
            return Ok(loaded);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf") {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.load_font_file(&name, &path, backend)?;
                    loaded.push(name);
                }
            }
        }
        Ok(loaded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_engine_new() {
        let engine = FontEngine::new();
        assert!(!engine.has_font("test"));
    }

    #[test]
    fn test_fallback_chain() {
        let mut engine = FontEngine::new();
        engine.add_fallback("heading", "GothamBold");
        engine.add_fallback("body", "GothamBook");
        // Without any fonts loaded, resolve returns the mapping target
        // but get_font_id returns None
        assert_eq!(engine.resolve("heading"), "GothamBold");
        assert!(engine.get_font_id("heading").is_none());
    }

    #[test]
    fn test_measure_text_fallback() {
        let engine = FontEngine::new();
        // Without loaded font, uses rough fallback
        let w = engine.measure_text("Hello", "nonexistent", 12.0);
        assert!(w > 0.0);
    }
}
