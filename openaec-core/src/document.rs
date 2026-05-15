//! Document model — page formats, margins, document configuration.
//!
//! Port of `core/document.py`. Defines the physical page layout in millimeters
//! and provides conversions to points for the layout engine.

use openaec_layout::Pt;
use serde::{Deserialize, Serialize};

/// Conversion factor: millimeters to points (1 pt = 1/72 inch).
pub const MM_TO_PT: f64 = 2.834_645_669_3;

// ── Page format ────────────────────────────────────────────────────────

/// Page format definition in millimeters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageFormatMm {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PageFormatMm {
    /// Width in points.
    pub fn width_pt(&self) -> Pt {
        Pt((self.width_mm * MM_TO_PT) as f32)
    }

    /// Height in points.
    pub fn height_pt(&self) -> Pt {
        Pt((self.height_mm * MM_TO_PT) as f32)
    }

    /// ReportLab-compatible (width, height) in points.
    pub fn size_pt(&self) -> (Pt, Pt) {
        (self.width_pt(), self.height_pt())
    }
}

/// Standard A4 page format.
pub const PAGE_A4: PageFormatMm = PageFormatMm {
    name: String::new(), // const requires empty; use `a4()` for named.
    width_mm: 210.0,
    height_mm: 297.0,
};

/// Standard A3 page format.
pub const PAGE_A3: PageFormatMm = PageFormatMm {
    name: String::new(),
    width_mm: 297.0,
    height_mm: 420.0,
};

impl PageFormatMm {
    /// Create a named A4 page format.
    pub fn a4() -> Self {
        Self {
            name: "A4".to_string(),
            width_mm: 210.0,
            height_mm: 297.0,
        }
    }

    /// Create a named A3 page format.
    pub fn a3() -> Self {
        Self {
            name: "A3".to_string(),
            width_mm: 297.0,
            height_mm: 420.0,
        }
    }
}

// ── Margins ────────────────────────────────────────────────────────────

/// Page margins in millimeters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 25.0,
            bottom: 20.0,
            left: 20.0,
            right: 15.0,
        }
    }
}

impl Margins {
    pub fn top_pt(&self) -> Pt {
        Pt((self.top * MM_TO_PT) as f32)
    }

    pub fn bottom_pt(&self) -> Pt {
        Pt((self.bottom * MM_TO_PT) as f32)
    }

    pub fn left_pt(&self) -> Pt {
        Pt((self.left * MM_TO_PT) as f32)
    }

    pub fn right_pt(&self) -> Pt {
        Pt((self.right * MM_TO_PT) as f32)
    }
}

// ── Document configuration ─────────────────────────────────────────────

/// Document orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DocOrientation {
    #[default]
    Portrait,
    Landscape,
}


/// Full document configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentConfig {
    pub format: PageFormatMm,
    pub margins: Margins,
    pub orientation: DocOrientation,

    // Project information
    pub project: String,
    pub project_number: String,
    pub client: String,
    pub author: String,
    pub report_type: String,
    pub subtitle: String,
}

impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            format: PageFormatMm::a4(),
            margins: Margins::default(),
            orientation: DocOrientation::Portrait,
            project: String::new(),
            project_number: String::new(),
            client: String::new(),
            author: "3BM Bouwkunde".to_string(),
            report_type: String::new(),
            subtitle: String::new(),
        }
    }
}

impl DocumentConfig {
    /// Effective page size in points, accounting for orientation.
    pub fn effective_page_size(&self) -> (Pt, Pt) {
        match self.orientation {
            DocOrientation::Landscape => (self.format.height_pt(), self.format.width_pt()),
            DocOrientation::Portrait => self.format.size_pt(),
        }
    }

    /// Effective page width in points.
    pub fn effective_width_pt(&self) -> Pt {
        self.effective_page_size().0
    }

    /// Effective page height in points.
    pub fn effective_height_pt(&self) -> Pt {
        self.effective_page_size().1
    }

    /// Available content width in points (page width minus left+right margins).
    pub fn content_width_pt(&self) -> Pt {
        Pt(self.effective_width_pt().0 - self.margins.left_pt().0 - self.margins.right_pt().0)
    }

    /// Available content height in points (page height minus top+bottom margins).
    pub fn content_height_pt(&self) -> Pt {
        Pt(self.effective_height_pt().0 - self.margins.top_pt().0 - self.margins.bottom_pt().0)
    }
}

/// Convert millimeters to points.
pub fn mm_to_pt(mm: f64) -> Pt {
    Pt((mm * MM_TO_PT) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a4_dimensions() {
        let a4 = PageFormatMm::a4();
        assert_eq!(a4.width_mm, 210.0);
        assert_eq!(a4.height_mm, 297.0);
        // A4 width ≈ 595.28 pt
        assert!((a4.width_pt().0 - 595.276).abs() < 0.01);
    }

    #[test]
    fn test_a3_dimensions() {
        let a3 = PageFormatMm::a3();
        assert_eq!(a3.width_mm, 297.0);
        assert_eq!(a3.height_mm, 420.0);
    }

    #[test]
    fn test_default_margins() {
        let m = Margins::default();
        assert_eq!(m.top, 25.0);
        assert_eq!(m.bottom, 20.0);
        assert_eq!(m.left, 20.0);
        assert_eq!(m.right, 15.0);
    }

    #[test]
    fn test_margin_pt_conversion() {
        let m = Margins::default();
        assert!((m.top_pt().0 - 70.866).abs() < 0.01);
    }

    #[test]
    fn test_document_config_portrait() {
        let cfg = DocumentConfig::default();
        let (w, h) = cfg.effective_page_size();
        // Portrait A4
        assert!((w.0 - 595.276).abs() < 0.01);
        assert!((h.0 - 841.890).abs() < 0.01);
    }

    #[test]
    fn test_document_config_landscape() {
        let cfg = DocumentConfig {
            orientation: DocOrientation::Landscape,
            ..Default::default()
        };
        let (w, h) = cfg.effective_page_size();
        // Landscape: width and height swapped
        assert!((w.0 - 841.890).abs() < 0.01);
        assert!((h.0 - 595.276).abs() < 0.01);
    }

    #[test]
    fn test_content_dimensions() {
        let cfg = DocumentConfig::default();
        let cw = cfg.content_width_pt();
        let ch = cfg.content_height_pt();
        // Content width = 210 - 20 - 15 = 175mm ≈ 496.06 pt
        assert!((cw.0 - 496.063).abs() < 0.1);
        // Content height = 297 - 25 - 20 = 252mm ≈ 714.33 pt
        assert!((ch.0 - 714.330).abs() < 0.1);
    }
}
