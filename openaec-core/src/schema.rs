//! Serde types matching `schemas/report.schema.json`.
//!
//! This module is the Rust-side contract between the library, API, and frontend.
//! Every struct maps 1:1 to the JSON schema definitions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Top-level report ──────────────────────────────────────────────────

/// Root report definition — corresponds to the top-level JSON object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    /// Template name (maps to YAML in templates/).
    pub template: String,

    /// Project name.
    pub project: String,

    /// Tenant identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,

    /// Paper format.
    #[serde(default = "default_format")]
    pub format: PaperFormat,

    /// Document orientation.
    #[serde(default)]
    pub orientation: Orientation,

    /// Project number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_number: Option<String>,

    /// Client name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,

    /// Author.
    #[serde(default = "default_author")]
    pub author: String,

    /// Report date (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,

    /// Version string.
    #[serde(default = "default_version")]
    pub version: String,

    /// Document status.
    #[serde(default)]
    pub status: ReportStatus,

    /// Cover page configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<Cover>,

    /// Colofon page configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colofon: Option<Colofon>,

    /// Table of contents configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toc: Option<TocConfig>,

    /// Report sections in order.
    #[serde(default)]
    pub sections: Vec<Section>,

    /// Back cover configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backcover: Option<BackcoverConfig>,

    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

// ── Enums ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperFormat {
    A4,
    A3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReportStatus {
    #[default]
    Concept,
    Definitief,
    Revisie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TableStyle {
    #[default]
    Default,
    Minimal,
    Striped,
}

// ── Cover ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cover {
    /// Subtitle on the cover page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    /// Cover image (render, photo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageSource>,

    /// Extra key-value fields on the cover.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_fields: HashMap<String, String>,
}

// ── Colofon ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colofon {
    #[serde(default = "bool_true")]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opdrachtgever_naam: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opdrachtgever_contact: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opdrachtgever_adres: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_bedrijf: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_naam: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub normen: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentgegevens: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub datum: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fase: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_colofon: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub kenmerk: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_email: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_telefoon: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_functie: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub adviseur_registratie: Option<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_fields: HashMap<String, String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revision_history: Vec<RevisionEntry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclaimer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionEntry {
    pub version: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub description: String,
}

// ── TOC ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,

    #[serde(default = "default_toc_title")]
    pub title: String,

    #[serde(default = "default_toc_depth")]
    pub max_depth: u8,
}

// ── Backcover ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackcoverConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

// ── Section ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section title (appears in TOC).
    pub title: String,

    /// Heading level: 1=chapter, 2=paragraph, 3=subparagraph.
    #[serde(default = "default_level")]
    pub level: u8,

    /// Section content blocks.
    #[serde(default)]
    pub content: Vec<ContentBlock>,

    /// Override orientation for this section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,

    /// Force page break before this section.
    #[serde(default)]
    pub page_break_before: bool,
}

// ── Content Blocks ────────────────────────────────────────────────────

/// A single content element within a section.
///
/// Uses `#[serde(tag = "type")]` for internally tagged enum matching
/// the JSON schema's `oneOf` with `const` type discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Paragraph(ParagraphBlock),
    Calculation(CalculationBlock),
    Check(CheckBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Map(MapBlock),
    Spacer(SpacerBlock),
    PageBreak(PageBreakBlock),
    BulletList(BulletListBlock),
    #[serde(rename = "heading_2")]
    Heading2(Heading2Block),
    RawFlowable(RawFlowableBlock),
    BicTable(BicTableBlock),
    CostSummary(CostSummaryBlock),
    LocationDetail(LocationDetailBlock),
    ObjectDescription(ObjectDescriptionBlock),
    Spreadsheet(SpreadsheetBlock),
}

// ── Paragraph ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParagraphBlock {
    /// Text content. Supports basic markup: <b>, <i>, <sub>, <sup>.
    pub text: String,

    /// Style name from stylesheet.
    #[serde(default = "default_style_normal")]
    pub style: String,
}

// ── Calculation ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationBlock {
    /// Calculation title.
    pub title: String,

    /// Mathematical formula.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,

    /// Substituted values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitution: Option<String>,

    /// Computed result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    /// Unit of the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Norm reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

// ── Check ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckBlock {
    /// Description of the check.
    pub description: String,

    /// Required/limit value as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_value: Option<String>,

    /// Calculated value as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calculated_value: Option<String>,

    /// Unity check value (0.0 = 0%, 1.0 = 100%).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unity_check: Option<f64>,

    /// UC limit (default 1.0).
    #[serde(default = "default_limit")]
    pub limit: f64,

    /// Explicit result. If omitted, computed from unity_check vs limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<CheckResult>,

    /// Norm reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckResult {
    #[serde(rename = "VOLDOET")]
    Pass,
    #[serde(rename = "VOLDOET NIET")]
    Fail,
}

impl CheckBlock {
    /// Determine check result: explicit result, or computed from UC.
    pub fn effective_result(&self) -> Option<CheckResult> {
        if let Some(result) = self.result {
            return Some(result);
        }
        self.unity_check.map(|uc| {
            if uc <= self.limit {
                CheckResult::Pass
            } else {
                CheckResult::Fail
            }
        })
    }
}

// ── Table ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBlock {
    /// Optional table title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Column headers.
    pub headers: Vec<String>,

    /// Data rows (2D array, cells can be any JSON value).
    pub rows: Vec<Vec<serde_json::Value>>,

    /// Column widths in mm (auto if omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_widths: Option<Vec<f64>>,

    /// Table style.
    #[serde(default)]
    pub style: TableStyle,
}

// ── Image ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageBlock {
    /// Image source (path, URL, or base64).
    pub src: ImageSource,

    /// Caption text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,

    /// Width in mm (auto-fit if omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,

    /// Horizontal alignment.
    #[serde(default)]
    pub alignment: Alignment,
}

// ── Map ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapBlock {
    /// Center point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<MapCenter>,

    /// Bounding box (alternative to center + radius).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<MapBbox>,

    /// Radius in meters around center.
    #[serde(default = "default_radius")]
    pub radius_m: f64,

    /// Map layers to render.
    #[serde(default = "default_map_layers")]
    pub layers: Vec<MapLayer>,

    /// Width in mm.
    #[serde(default = "default_map_width")]
    pub width_mm: f64,

    /// Caption text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapCenter {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapBbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapLayer {
    Percelen,
    Bebouwing,
    Bestemmingsplan,
    Luchtfoto,
}

// ── Spacer ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacerBlock {
    /// Height in mm.
    #[serde(default = "default_spacer_height")]
    pub height_mm: f64,
}

// ── Page Break ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageBreakBlock {}

// ── Bullet List ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletListBlock {
    /// List items.
    pub items: Vec<String>,
}

// ── Heading 2 ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading2Block {
    /// Section number (e.g. "1.1").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,

    /// Heading title.
    pub title: String,
}

// ── Raw Flowable (library-only, not via API) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFlowableBlock {
    pub class_name: String,

    #[serde(default)]
    pub kwargs: HashMap<String, serde_json::Value>,
}

// ── BIC Table (Symitech tenant) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BicTableBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_name: Option<String>,

    #[serde(default)]
    pub sections: Vec<BicSection>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BicSection {
    pub title: String,
    pub rows: Vec<BicRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BicRow {
    pub label: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_value: Option<String>,

    #[serde(default)]
    pub is_currency: bool,
}

// ── Cost Summary (Symitech tenant) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummaryBlock {
    #[serde(default)]
    pub columns: Vec<String>,

    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<CostTotal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTotal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<serde_json::Value>,
}

// ── Location Detail (Symitech tenant) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationDetailBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<LocationClient>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationInfo>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_path: Option<ImageSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationClient {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cadastral: Option<String>,
}

// ── Object Description (Symitech tenant) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDescriptionBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,

    #[serde(default)]
    pub fields: Vec<ObjectField>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_path: Option<ImageSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectField {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

// ── Spreadsheet ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadsheetBlock {
    /// Optional table title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Column headers.
    #[serde(default)]
    pub headers: Vec<String>,

    /// Data rows.
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,

    /// Show row numbers.
    #[serde(default)]
    pub show_row_numbers: bool,

    /// Optional footnote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnote: Option<String>,

    /// Column widths in mm (auto if omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_widths: Option<Vec<f64>>,
}

// ── Image Source ───────────────────────────────────────────────────────

/// Image source — supports file path/URL (string) or base64 encoded data (object).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageSource {
    /// File path (relative or absolute) or URL.
    Path(String),

    /// Base64 encoded image data.
    Base64 {
        /// Base64 encoded image bytes.
        data: String,
        /// MIME type.
        media_type: MediaType,
        /// Original filename (optional).
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/svg+xml")]
    Svg,
}

// ── Default helpers ───────────────────────────────────────────────────

fn default_format() -> PaperFormat {
    PaperFormat::A4
}

fn default_author() -> String {
    "3BM Bouwkunde".to_string()
}

fn default_version() -> String {
    "1.0".to_string()
}

fn bool_true() -> bool {
    true
}

fn default_toc_title() -> String {
    "Inhoudsopgave".to_string()
}

fn default_toc_depth() -> u8 {
    3
}

fn default_level() -> u8 {
    1
}

fn default_style_normal() -> String {
    "Normal".to_string()
}

fn default_limit() -> f64 {
    1.0
}

fn default_radius() -> f64 {
    100.0
}

fn default_map_layers() -> Vec<MapLayer> {
    vec![MapLayer::Percelen, MapLayer::Bebouwing]
}

fn default_map_width() -> f64 {
    170.0
}

fn default_spacer_height() -> f64 {
    5.0
}

// ── Convenience constructors ──────────────────────────────────────────

impl ReportData {
    /// Deserialize from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Deserialize from a JSON file.
    pub fn from_json_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Serialize to a JSON string (pretty-printed).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_report() {
        let json = r#"{
            "template": "structural",
            "project": "Test Project"
        }"#;

        let report: ReportData = serde_json::from_str(json).unwrap();
        assert_eq!(report.template, "structural");
        assert_eq!(report.project, "Test Project");
        assert_eq!(report.format, PaperFormat::A4);
        assert_eq!(report.orientation, Orientation::Portrait);
        assert_eq!(report.status, ReportStatus::Concept);
        assert_eq!(report.author, "3BM Bouwkunde");
        assert_eq!(report.version, "1.0");
        assert!(report.sections.is_empty());
    }

    #[test]
    fn test_paragraph_block() {
        let json = r#"{
            "type": "paragraph",
            "text": "Hello <b>world</b>"
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Paragraph(p) => {
                assert_eq!(p.text, "Hello <b>world</b>");
                assert_eq!(p.style, "Normal");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_calculation_block() {
        let json = r#"{
            "type": "calculation",
            "title": "Buigend moment",
            "formula": "M_Ed = q * l^2 / 8",
            "substitution": "M_Ed = 8.5 * 6.0^2 / 8",
            "result": "38.3",
            "unit": "kNm",
            "reference": "NEN-EN 1992-1-1 §6.1"
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Calculation(c) => {
                assert_eq!(c.title, "Buigend moment");
                assert_eq!(c.unit.as_deref(), Some("kNm"));
            }
            _ => panic!("Expected Calculation"),
        }
    }

    #[test]
    fn test_check_block_pass() {
        let json = r#"{
            "type": "check",
            "description": "Doorbuigingscontrole",
            "required_value": "l/250 = 24.0 mm",
            "calculated_value": "18.5 mm",
            "unity_check": 0.77,
            "limit": 1.0
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Check(c) => {
                assert_eq!(c.effective_result(), Some(CheckResult::Pass));
            }
            _ => panic!("Expected Check"),
        }
    }

    #[test]
    fn test_check_block_fail() {
        let json = r#"{
            "type": "check",
            "description": "Overbelasting",
            "unity_check": 1.15
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Check(c) => {
                assert_eq!(c.effective_result(), Some(CheckResult::Fail));
            }
            _ => panic!("Expected Check"),
        }
    }

    #[test]
    fn test_table_block() {
        let json = r#"{
            "type": "table",
            "headers": ["Element", "Profiel", "UC"],
            "rows": [
                ["HEA200", "S355", 0.85],
                ["IPE300", "S235", 0.92]
            ],
            "column_widths": [60, 60, 40]
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Table(t) => {
                assert_eq!(t.headers.len(), 3);
                assert_eq!(t.rows.len(), 2);
                assert_eq!(t.column_widths.as_ref().unwrap().len(), 3);
            }
            _ => panic!("Expected Table"),
        }
    }

    #[test]
    fn test_image_source_path() {
        let json = r#"{
            "type": "image",
            "src": "images/render.png",
            "caption": "3D render"
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Image(img) => {
                assert!(matches!(img.src, ImageSource::Path(_)));
                assert_eq!(img.caption.as_deref(), Some("3D render"));
            }
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_image_source_base64() {
        let json = r#"{
            "type": "image",
            "src": {
                "data": "iVBORw0KGgo=",
                "media_type": "image/png",
                "filename": "test.png"
            }
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Image(img) => {
                assert!(matches!(img.src, ImageSource::Base64 { .. }));
            }
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_map_block() {
        let json = r#"{
            "type": "map",
            "center": { "lat": 52.0907, "lon": 5.1214 },
            "radius_m": 200,
            "layers": ["percelen", "bebouwing", "luchtfoto"]
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Map(m) => {
                assert!(m.center.is_some());
                assert_eq!(m.radius_m, 200.0);
                assert_eq!(m.layers.len(), 3);
            }
            _ => panic!("Expected Map"),
        }
    }

    #[test]
    fn test_bullet_list_block() {
        let json = r#"{
            "type": "bullet_list",
            "items": ["Item 1", "Item 2", "Item 3"]
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::BulletList(bl) => {
                assert_eq!(bl.items.len(), 3);
            }
            _ => panic!("Expected BulletList"),
        }
    }

    #[test]
    fn test_heading2_block() {
        let json = r#"{
            "type": "heading_2",
            "number": "1.1",
            "title": "Uitgangspunten"
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Heading2(h) => {
                assert_eq!(h.number.as_deref(), Some("1.1"));
                assert_eq!(h.title, "Uitgangspunten");
            }
            _ => panic!("Expected Heading2"),
        }
    }

    #[test]
    fn test_spacer_and_page_break() {
        let spacer_json = r#"{ "type": "spacer", "height_mm": 10 }"#;
        let pb_json = r#"{ "type": "page_break" }"#;

        let spacer: ContentBlock = serde_json::from_str(spacer_json).unwrap();
        assert!(matches!(spacer, ContentBlock::Spacer(_)));

        let pb: ContentBlock = serde_json::from_str(pb_json).unwrap();
        assert!(matches!(pb, ContentBlock::PageBreak(_)));
    }

    #[test]
    fn test_section_with_content() {
        let json = r#"{
            "title": "Constructieve uitgangspunten",
            "level": 1,
            "content": [
                { "type": "paragraph", "text": "De berekening is uitgevoerd conform de Eurocode." },
                { "type": "spacer", "height_mm": 5 },
                { "type": "bullet_list", "items": ["NEN-EN 1990", "NEN-EN 1991-1-1"] }
            ]
        }"#;

        let section: Section = serde_json::from_str(json).unwrap();
        assert_eq!(section.title, "Constructieve uitgangspunten");
        assert_eq!(section.content.len(), 3);
    }

    #[test]
    fn test_colofon() {
        let json = r#"{
            "enabled": true,
            "opdrachtgever_naam": "Gemeente Den Haag",
            "adviseur_bedrijf": "3BM Bouwkunde",
            "adviseur_naam": "J. Kragten",
            "revision_history": [
                {
                    "version": "1.0",
                    "date": "2026-02-18",
                    "author": "JK",
                    "description": "Eerste uitgave"
                }
            ]
        }"#;

        let colofon: Colofon = serde_json::from_str(json).unwrap();
        assert!(colofon.enabled);
        assert_eq!(
            colofon.opdrachtgever_naam.as_deref(),
            Some("Gemeente Den Haag")
        );
        assert_eq!(colofon.revision_history.len(), 1);
    }

    #[test]
    fn test_report_roundtrip() {
        let report = ReportData {
            template: "structural".to_string(),
            project: "Roundtrip Test".to_string(),
            tenant: None,
            format: PaperFormat::A4,
            orientation: Orientation::Portrait,
            project_number: Some("2026-001".to_string()),
            client: Some("Test Client".to_string()),
            author: "3BM Bouwkunde".to_string(),
            date: Some("2026-03-04".to_string()),
            version: "1.0".to_string(),
            status: ReportStatus::Concept,
            cover: None,
            colofon: None,
            toc: None,
            sections: vec![],
            backcover: None,
            metadata: HashMap::new(),
        };

        let json = report.to_json().unwrap();
        let parsed: ReportData = ReportData::from_json(&json).unwrap();
        assert_eq!(parsed.project, "Roundtrip Test");
        assert_eq!(parsed.project_number.as_deref(), Some("2026-001"));
    }

    #[test]
    fn test_bic_table_block() {
        let json = r#"{
            "type": "bic_table",
            "location_name": "Kantoor A",
            "sections": [
                {
                    "title": "Brandveiligheid",
                    "rows": [
                        { "label": "Rookmelders", "ref_value": "Ja", "actual_value": "Ja" }
                    ]
                }
            ]
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(matches!(block, ContentBlock::BicTable(_)));
    }

    #[test]
    fn test_check_explicit_result() {
        let json = r#"{
            "type": "check",
            "description": "Handmatige beoordeling",
            "result": "VOLDOET"
        }"#;

        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Check(c) => {
                assert_eq!(c.effective_result(), Some(CheckResult::Pass));
            }
            _ => panic!("Expected Check"),
        }
    }
}
