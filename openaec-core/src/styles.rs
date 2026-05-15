//! Style system — colors, fonts, spacing, paragraph styles.
//!
//! Port of `core/styles.py`. Provides `StyleSet` as an explicit parameter
//! (no global mutable state). Produces `openaec_layout::ParagraphStyle` objects.

use openaec_layout::{Color, ParagraphStyle, Pt};
use serde::{Deserialize, Serialize};

use crate::brand::BrandConfig;

/// Shared padding constant for content block components (in points).
pub const BLOCK_PADDING: f64 = 6.0;

// ── Colors ─────────────────────────────────────────────────────────────

/// 3BM brand color palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub warning: String,
    pub text: String,
    pub text_accent: String,
    pub text_light: String,
    pub background: String,
    pub background_alt: String,
    pub rule: String,
    pub table_header_bg: String,
    pub table_header_text: String,
    pub table_footer_bg: String,
    pub separator: String,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            primary: "#40124A".to_string(),
            secondary: "#38BDA0".to_string(),
            accent: "#2ECC71".to_string(),
            warning: "#E74C3C".to_string(),
            text: "#45243D".to_string(),
            text_accent: "#56B49B".to_string(),
            text_light: "#7F8C8D".to_string(),
            background: "#FFFFFF".to_string(),
            background_alt: "#F8F9FA".to_string(),
            rule: "#BDC3C7".to_string(),
            table_header_bg: "#45233C".to_string(),
            table_header_text: "#FFFFFF".to_string(),
            table_footer_bg: "#55B49B".to_string(),
            separator: "#E0D0E8".to_string(),
        }
    }
}

impl Colors {
    /// Resolve a color name to a `Color`.
    pub fn resolve(&self, name: &str) -> Option<Color> {
        let hex = match name {
            "primary" => &self.primary,
            "secondary" => &self.secondary,
            "accent" => &self.accent,
            "warning" => &self.warning,
            "text" => &self.text,
            "text_accent" => &self.text_accent,
            "text_light" => &self.text_light,
            "background" => &self.background,
            "background_alt" => &self.background_alt,
            "rule" => &self.rule,
            "table_header_bg" => &self.table_header_bg,
            "table_header_text" => &self.table_header_text,
            "table_footer_bg" => &self.table_footer_bg,
            "separator" => &self.separator,
            other if other.starts_with('#') => other,
            _ => return None,
        };
        Color::from_hex(hex)
    }
}

// ── Font configuration ─────────────────────────────────────────────────

/// Font role configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    pub heading: String,
    pub body: String,
    pub medium: String,
    pub italic: String,
    pub mono: String,
    pub body_size: f64,
    pub heading1_size: f64,
    pub heading2_size: f64,
    pub heading3_size: f64,
    pub caption_size: f64,
    pub footer_size: f64,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            heading: "GothamBold".to_string(),
            body: "GothamBook".to_string(),
            medium: "GothamMedium".to_string(),
            italic: "GothamBookItalic".to_string(),
            mono: "Courier".to_string(),
            body_size: 9.5,
            heading1_size: 18.0,
            heading2_size: 13.0,
            heading3_size: 11.0,
            caption_size: 8.0,
            footer_size: 7.5,
        }
    }
}

// ── StyleSet ───────────────────────────────────────────────────────────

/// Complete style set — colors, fonts, and derived paragraph styles.
///
/// Passed explicitly as a parameter (no global state). Created from a
/// `BrandConfig` or using 3BM defaults.
#[derive(Debug, Clone)]
pub struct StyleSet {
    pub colors: Colors,
    pub fonts: FontConfig,
}

impl Default for StyleSet {
    fn default() -> Self {
        Self::default_3bm()
    }
}

impl StyleSet {
    /// Create a `StyleSet` with 3BM default colors and fonts.
    pub fn default_3bm() -> Self {
        Self {
            colors: Colors::default(),
            fonts: FontConfig::default(),
        }
    }

    /// Create a `StyleSet` from a `BrandConfig`.
    pub fn from_brand(brand: &BrandConfig) -> Self {
        let mut colors = Colors::default();

        // Apply brand colors
        if let Some(v) = brand.colors.get("primary") {
            colors.primary = v.clone();
        }
        if let Some(v) = brand.colors.get("secondary") {
            colors.secondary = v.clone();
        }
        if let Some(v) = brand.colors.get("accent") {
            colors.accent = v.clone();
        }
        if let Some(v) = brand.colors.get("warning") {
            colors.warning = v.clone();
        }
        if let Some(v) = brand.colors.get("text") {
            colors.text = v.clone();
        }
        if let Some(v) = brand.colors.get("text_accent") {
            colors.text_accent = v.clone();
        }
        if let Some(v) = brand.colors.get("text_light") {
            colors.text_light = v.clone();
        }
        if let Some(v) = brand.colors.get("background") {
            colors.background = v.clone();
        }
        if let Some(v) = brand.colors.get("background_alt") {
            colors.background_alt = v.clone();
        }
        if let Some(v) = brand.colors.get("table_header_bg") {
            colors.table_header_bg = v.clone();
        }
        if let Some(v) = brand.colors.get("table_header_text") {
            colors.table_header_text = v.clone();
        }
        if let Some(v) = brand.colors.get("table_footer_bg") {
            colors.table_footer_bg = v.clone();
        }
        if let Some(v) = brand.colors.get("separator") {
            colors.separator = v.clone();
        }

        // Apply brand fonts
        let mut fonts = FontConfig::default();
        if let Some(v) = brand.fonts.get("heading") {
            fonts.heading = v.clone();
        }
        if let Some(v) = brand.fonts.get("body") {
            fonts.body = v.clone();
        }
        if let Some(v) = brand.fonts.get("medium") {
            fonts.medium = v.clone();
        }
        if let Some(v) = brand.fonts.get("italic") {
            fonts.italic = v.clone();
        }
        if let Some(v) = brand.fonts.get("mono") {
            fonts.mono = v.clone();
        }

        Self { colors, fonts }
    }

    // ── Paragraph style factories ──────────────────────────────────────

    /// Normal body text style.
    pub fn normal(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.body_size as f32),
            leading: Pt((self.fonts.body_size * 1.4) as f32),
            text_color: self.color_or_default(&self.colors.text, Color::BLACK),
            space_after: Pt(4.0),
            ..Default::default()
        }
    }

    /// Heading 1 style.
    pub fn heading1(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.heading1_size as f32),
            leading: Pt((self.fonts.heading1_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text, Color::BLACK),
            bold: false,
            space_before: Pt(12.0),
            space_after: Pt(6.0),
            ..Default::default()
        }
    }

    /// Heading 2 style.
    pub fn heading2(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.heading2_size as f32),
            leading: Pt((self.fonts.heading2_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text_accent, Color::BLACK),
            bold: false,
            space_before: Pt(10.0),
            space_after: Pt(4.0),
            ..Default::default()
        }
    }

    /// Heading 3 style.
    pub fn heading3(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.heading3_size as f32),
            leading: Pt((self.fonts.heading3_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text_accent, Color::BLACK),
            bold: false,
            space_before: Pt(8.0),
            space_after: Pt(3.0),
            ..Default::default()
        }
    }

    /// Caption style — small, centered, light grey.
    pub fn caption(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.caption_size as f32),
            leading: Pt((self.fonts.caption_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text_light, Color::GREY),
            ..Default::default()
        }
    }

    /// Footer style.
    pub fn footer(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.footer_size as f32),
            leading: Pt((self.fonts.footer_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text_light, Color::GREY),
            ..Default::default()
        }
    }

    /// Cover title style.
    pub fn cover_title(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(28.0),
            leading: Pt(34.0),
            bold: true,
            text_color: self.color_or_default(&self.colors.primary, Color::BLACK),
            ..Default::default()
        }
    }

    /// Cover subtitle style.
    pub fn cover_subtitle(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(14.0),
            leading: Pt(18.0),
            text_color: self.color_or_default(&self.colors.secondary, Color::BLACK),
            ..Default::default()
        }
    }

    // ── Block-specific styles ──────────────────────────────────────────

    /// Reference style — small, right-aligned, light grey.
    pub fn block_reference(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.caption_size as f32),
            leading: Pt((self.fonts.caption_size * 1.3) as f32),
            text_color: self.color_or_default(&self.colors.text_light, Color::GREY),
            space_after: Pt(0.0),
            ..Default::default()
        }
    }

    /// Block heading style — bold, body size, primary color.
    pub fn block_heading(&self, text_color: Option<&str>) -> ParagraphStyle {
        let color = text_color
            .and_then(Color::from_hex)
            .unwrap_or_else(|| {
                self.color_or_default(&self.colors.primary, Color::BLACK)
            });
        ParagraphStyle {
            font_size: Pt(self.fonts.body_size as f32),
            leading: Pt((self.fonts.body_size * 1.3) as f32),
            bold: true,
            text_color: color,
            space_after: Pt(0.0),
            ..Default::default()
        }
    }

    /// Block body style — normal, slightly more leading.
    pub fn block_body(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.body_size as f32),
            leading: Pt((self.fonts.body_size * 1.4) as f32),
            text_color: self.color_or_default(&self.colors.text, Color::BLACK),
            space_after: Pt(0.0),
            ..Default::default()
        }
    }

    /// Block monospace style — Courier, body size.
    pub fn block_mono(&self) -> ParagraphStyle {
        ParagraphStyle {
            font_size: Pt(self.fonts.body_size as f32),
            leading: Pt((self.fonts.body_size * 1.4) as f32),
            text_color: self.color_or_default(&self.colors.text, Color::BLACK),
            space_after: Pt(0.0),
            ..Default::default()
        }
    }

    /// Block result style — bold, slightly larger, primary color.
    pub fn block_result(&self, text_color: Option<&str>) -> ParagraphStyle {
        let color = text_color
            .and_then(Color::from_hex)
            .unwrap_or_else(|| {
                self.color_or_default(&self.colors.primary, Color::BLACK)
            });
        let size = self.fonts.body_size + 1.0;
        ParagraphStyle {
            font_size: Pt(size as f32),
            leading: Pt((size * 1.3) as f32),
            bold: true,
            text_color: color,
            space_after: Pt(0.0),
            ..Default::default()
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn color_or_default(&self, hex: &str, fallback: Color) -> Color {
        Color::from_hex(hex).unwrap_or(fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_colors() {
        let c = Colors::default();
        assert_eq!(c.primary, "#40124A");
        assert_eq!(c.secondary, "#38BDA0");
        assert_eq!(c.text, "#45243D");
    }

    #[test]
    fn test_color_resolve() {
        let c = Colors::default();
        assert!(c.resolve("primary").is_some());
        assert!(c.resolve("text").is_some());
        assert!(c.resolve("#FF0000").is_some());
        assert!(c.resolve("nonexistent").is_none());
    }

    #[test]
    fn test_default_fonts() {
        let f = FontConfig::default();
        assert_eq!(f.heading, "GothamBold");
        assert_eq!(f.body, "GothamBook");
        assert_eq!(f.body_size, 9.5);
    }

    #[test]
    fn test_styleset_default() {
        let ss = StyleSet::default_3bm();
        assert_eq!(ss.colors.primary, "#40124A");
        assert_eq!(ss.fonts.heading, "GothamBold");
    }

    #[test]
    fn test_styleset_from_brand() {
        let mut brand = BrandConfig::default();
        brand
            .colors
            .insert("primary".to_string(), "#FF0000".to_string());
        brand
            .fonts
            .insert("heading".to_string(), "CustomBold".to_string());

        let ss = StyleSet::from_brand(&brand);
        assert_eq!(ss.colors.primary, "#FF0000");
        assert_eq!(ss.fonts.heading, "CustomBold");
        // Unchanged defaults
        assert_eq!(ss.colors.secondary, "#38BDA0");
    }

    #[test]
    fn test_normal_style() {
        let ss = StyleSet::default_3bm();
        let style = ss.normal();
        assert_eq!(style.font_size, Pt(9.5));
        assert_eq!(style.space_after, Pt(4.0));
    }

    #[test]
    fn test_heading_styles() {
        let ss = StyleSet::default_3bm();
        let h1 = ss.heading1();
        let h2 = ss.heading2();
        assert_eq!(h1.font_size, Pt(18.0));
        assert_eq!(h2.font_size, Pt(13.0));
    }

    #[test]
    fn test_block_heading_custom_color() {
        let ss = StyleSet::default_3bm();
        let style = ss.block_heading(Some("#FF0000"));
        // Should use the provided color
        assert_eq!(style.text_color, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_block_padding_constant() {
        assert_eq!(BLOCK_PADDING, 6.0);
    }
}
