//! Template-driven report configuration.
//!
//! Port of `core/template_config.py`. Dataclasses for loading template YAMLs
//! and page_type YAMLs. Templates define document structure (page order).
//! Page types define what goes on each page.

use serde::{Deserialize, Serialize};

/// Text field at a fixed position on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextZone {
    /// Dot-notation path in data: "client.name"
    pub bind: String,
    #[serde(default)]
    pub x_mm: f64,
    #[serde(default)]
    pub y_mm: f64,
    #[serde(default = "default_font_body")]
    pub font: String,
    #[serde(default = "default_size_10")]
    pub size: f64,
    #[serde(default = "default_color_text")]
    pub color: String,
    #[serde(default = "default_align_left")]
    pub align: TextAlign,
    /// Maximale breedte — tekst wraps bij overschrijding.
    #[serde(default)]
    pub max_width_mm: Option<f64>,
    /// Regelafstand voor multi-line tekst (default 4.2 mm).
    #[serde(default = "default_line_height")]
    pub line_height_mm: f64,
}

/// Image at a fixed position on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageZone {
    /// Dot-notation path to image (path or base64)
    pub bind: String,
    #[serde(default)]
    pub x_mm: f64,
    #[serde(default)]
    pub y_mm: f64,
    #[serde(default = "default_100")]
    pub width_mm: f64,
    #[serde(default = "default_70")]
    pub height_mm: f64,
    #[serde(default)]
    pub fallback: String,
}

/// Decorative line at a fixed position on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineZone {
    #[serde(default)]
    pub x0_mm: f64,
    #[serde(default)]
    pub y_mm: f64,
    #[serde(default = "default_100")]
    pub x1_mm: f64,
    #[serde(default = "default_line_width")]
    pub width_pt: f64,
    #[serde(default = "default_color_primary")]
    pub color: String,
}

/// Column definition for a fixed-page table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    /// Key in data dict
    pub field: String,
    #[serde(default = "default_40")]
    pub width_mm: f64,
    #[serde(default = "default_align_left")]
    pub align: TextAlign,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default = "default_font_body")]
    pub font: String,
    #[serde(default = "default_size_9")]
    pub size: f64,
    #[serde(default = "default_color_text")]
    pub color: String,
    /// Display name for column header (fallback: field)
    #[serde(default)]
    pub header: Option<String>,
}

/// Table configuration for fixed pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    /// Dot-notation path to list in data
    pub data_bind: String,
    #[serde(default)]
    pub columns: Vec<TableColumn>,
    #[serde(default = "default_20")]
    pub origin_x_mm: f64,
    #[serde(default = "default_60")]
    pub origin_y_mm: f64,
    #[serde(default = "default_row_height")]
    pub row_height_mm: f64,
    #[serde(default = "default_260")]
    pub max_y_mm: f64,
    #[serde(default = "default_font_heading")]
    pub header_font: String,
    #[serde(default = "default_size_9")]
    pub header_size: f64,
    #[serde(default = "default_color_text")]
    pub header_color: String,
    #[serde(default)]
    pub show_header: bool,
    #[serde(default)]
    pub header_bg: Option<String>,
    #[serde(default)]
    pub body_font: Option<String>,
    #[serde(default)]
    pub body_size: Option<f64>,
    #[serde(default)]
    pub body_color: Option<String>,
    #[serde(default)]
    pub alt_row_bg: Option<String>,
    #[serde(default)]
    pub grid_color: Option<String>,
}

/// Frame definition for flow-mode pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFrameDef {
    #[serde(default = "default_20")]
    pub x_mm: f64,
    #[serde(default = "default_25")]
    pub y_mm: f64,
    #[serde(default = "default_175")]
    pub width_mm: f64,
    #[serde(default = "default_247")]
    pub height_mm: f64,
}

impl Default for ContentFrameDef {
    fn default() -> Self {
        Self {
            x_mm: 20.0,
            y_mm: 25.0,
            width_mm: 175.0,
            height_mm: 247.0,
        }
    }
}

/// Definition of what goes on a page type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageType {
    pub name: String,
    #[serde(default)]
    pub stationery: Option<String>,
    #[serde(default)]
    pub text_zones: Vec<TextZone>,
    #[serde(default)]
    pub image_zones: Vec<ImageZone>,
    #[serde(default)]
    pub line_zones: Vec<LineZone>,
    #[serde(default)]
    pub table: Option<TableConfig>,
    #[serde(default)]
    pub content_frame: Option<ContentFrameDef>,
    /// Text zones verschuiven automatisch bij wrapping overflow.
    #[serde(default)]
    pub flow_layout: bool,
    /// Zones >= deze y zijn footer (vast, niet verschoven). Default 260.0 mm.
    #[serde(default = "default_flow_footer_y")]
    pub flow_footer_y_mm: f64,
    /// Y-start voor overflow vervolg-pagina's. Default 32.0 mm.
    #[serde(default = "default_flow_content_start_y")]
    pub flow_content_start_y_mm: f64,
}

/// Page definition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageDefType {
    Special,
    Fixed,
    Flow,
    Toc,
}

/// Page repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RepeatMode {
    Auto,
    #[default]
    None,
}


/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
}


/// Page definition in a template — references a page_type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDef {
    #[serde(rename = "type")]
    pub page_type_kind: PageDefType,
    /// Name → resolves to PageType
    pub page_type: String,
    #[serde(default = "default_orientation_portrait")]
    pub orientation: String,
    #[serde(default)]
    pub repeat: RepeatMode,
}

/// Document structure — template_engine format (v2).
///
/// Named `TemplateConfigV2` to avoid collision with the legacy
/// `TemplateConfig` in `template_loader.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfigV2 {
    pub name: String,
    #[serde(default)]
    pub tenant: String,
    #[serde(default)]
    pub pages: Vec<PageDef>,
}

// ── Default value helpers ──────────────────────────────────────────────

fn default_font_body() -> String {
    "body".to_string()
}

fn default_font_heading() -> String {
    "heading".to_string()
}

fn default_color_text() -> String {
    "text".to_string()
}

fn default_color_primary() -> String {
    "primary".to_string()
}

fn default_size_10() -> f64 {
    10.0
}

fn default_size_9() -> f64 {
    9.0
}

fn default_align_left() -> TextAlign {
    TextAlign::Left
}

fn default_orientation_portrait() -> String {
    "portrait".to_string()
}

fn default_100() -> f64 {
    100.0
}

fn default_70() -> f64 {
    70.0
}

fn default_40() -> f64 {
    40.0
}

fn default_20() -> f64 {
    20.0
}

fn default_25() -> f64 {
    25.0
}

fn default_60() -> f64 {
    60.0
}

fn default_175() -> f64 {
    175.0
}

fn default_247() -> f64 {
    247.0
}

fn default_260() -> f64 {
    260.0
}

fn default_row_height() -> f64 {
    5.6
}

fn default_line_width() -> f64 {
    1.0
}

fn default_line_height() -> f64 {
    4.2
}

fn default_flow_footer_y() -> f64 {
    260.0
}

fn default_flow_content_start_y() -> f64 {
    32.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_zone_deserialize() {
        let yaml = r#"
bind: "client.name"
x_mm: 25.0
y_mm: 42.0
font: heading
size: 12.0
color: primary
align: right
"#;
        let zone: TextZone = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(zone.bind, "client.name");
        assert_eq!(zone.x_mm, 25.0);
        assert_eq!(zone.font, "heading");
        assert_eq!(zone.align, TextAlign::Right);
    }

    #[test]
    fn test_text_zone_defaults() {
        let yaml = r#"bind: "test""#;
        let zone: TextZone = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(zone.font, "body");
        assert_eq!(zone.size, 10.0);
        assert_eq!(zone.color, "text");
        assert_eq!(zone.align, TextAlign::Left);
    }

    #[test]
    fn test_image_zone_deserialize() {
        let yaml = r#"
bind: "cover.image"
x_mm: 10.0
y_mm: 20.0
width_mm: 180.0
height_mm: 120.0
fallback: "default_cover.png"
"#;
        let zone: ImageZone = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(zone.bind, "cover.image");
        assert_eq!(zone.width_mm, 180.0);
        assert_eq!(zone.fallback, "default_cover.png");
    }

    #[test]
    fn test_line_zone_defaults() {
        let yaml = "{}";
        let zone: LineZone = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(zone.x1_mm, 100.0);
        assert_eq!(zone.width_pt, 1.0);
        assert_eq!(zone.color, "primary");
    }

    #[test]
    fn test_table_config_deserialize() {
        let yaml = r#"
data_bind: "objecten"
columns:
  - field: naam
    width_mm: 60.0
  - field: prijs
    width_mm: 30.0
    align: right
    format: currency_nl
origin_x_mm: 20.0
origin_y_mm: 55.0
show_header: true
"#;
        let table: TableConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(table.data_bind, "objecten");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].field, "naam");
        assert_eq!(table.columns[1].align, TextAlign::Right);
        assert!(table.show_header);
    }

    #[test]
    fn test_content_frame_defaults() {
        let frame = ContentFrameDef::default();
        assert_eq!(frame.x_mm, 20.0);
        assert_eq!(frame.y_mm, 25.0);
        assert_eq!(frame.width_mm, 175.0);
        assert_eq!(frame.height_mm, 247.0);
    }

    #[test]
    fn test_page_type_deserialize() {
        let yaml = r#"
name: cover
stationery: cover_bg.pdf
text_zones:
  - bind: project.name
    x_mm: 25.0
    y_mm: 100.0
    font: heading
    size: 18.0
image_zones: []
"#;
        let pt: PageType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pt.name, "cover");
        assert_eq!(pt.stationery.as_deref(), Some("cover_bg.pdf"));
        assert_eq!(pt.text_zones.len(), 1);
    }

    #[test]
    fn test_page_def_deserialize() {
        let yaml = r#"
type: flow
page_type: content
orientation: portrait
repeat: auto
"#;
        let pd: PageDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pd.page_type_kind, PageDefType::Flow);
        assert_eq!(pd.page_type, "content");
        assert_eq!(pd.repeat, RepeatMode::Auto);
    }

    #[test]
    fn test_text_zone_flow_fields() {
        let yaml = r#"
bind: "notes"
x_mm: 25.0
y_mm: 100.0
max_width_mm: 80.0
line_height_mm: 5.0
"#;
        let zone: TextZone = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(zone.max_width_mm, Some(80.0));
        assert_eq!(zone.line_height_mm, 5.0);

        // Defaults
        let yaml2 = r#"bind: "test""#;
        let zone2: TextZone = serde_yaml::from_str(yaml2).unwrap();
        assert!(zone2.max_width_mm.is_none());
        assert_eq!(zone2.line_height_mm, 4.2);
    }

    #[test]
    fn test_page_type_flow_layout_fields() {
        let yaml = r#"
name: locatie
flow_layout: true
flow_footer_y_mm: 255.0
flow_content_start_y_mm: 35.0
text_zones: []
"#;
        let pt: PageType = serde_yaml::from_str(yaml).unwrap();
        assert!(pt.flow_layout);
        assert_eq!(pt.flow_footer_y_mm, 255.0);
        assert_eq!(pt.flow_content_start_y_mm, 35.0);

        // Defaults
        let yaml2 = r#"name: cover"#;
        let pt2: PageType = serde_yaml::from_str(yaml2).unwrap();
        assert!(!pt2.flow_layout);
        assert_eq!(pt2.flow_footer_y_mm, 260.0);
        assert_eq!(pt2.flow_content_start_y_mm, 32.0);
    }

    #[test]
    fn test_template_config_v2() {
        let yaml = r#"
name: bic_factuur
tenant: 3bm_cooperatie
pages:
  - type: special
    page_type: cover
  - type: flow
    page_type: content
    repeat: auto
"#;
        let tc: TemplateConfigV2 = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tc.name, "bic_factuur");
        assert_eq!(tc.tenant, "3bm_cooperatie");
        assert_eq!(tc.pages.len(), 2);
        assert_eq!(tc.pages[0].page_type_kind, PageDefType::Special);
    }
}
