//! JSON adapter — generic data import from JSON files/strings.
//!
//! Port of `data/json_adapter.py`. The primary data interface for
//! the report generator. All other adapters (Revit, ERPNext) export
//! to this format.

use std::path::Path;

use serde_json::Value;

use crate::schema::ReportData;

/// Validation error.
#[derive(Debug, thiserror::Error)]
pub enum JsonAdapterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Loads and validates project data from JSON.
///
/// This is the primary data interface. All report data follows the
/// `report.schema.json` contract.
pub struct JsonAdapter {
    data: Value,
}

impl JsonAdapter {
    /// Create an empty adapter.
    pub fn new() -> Self {
        Self {
            data: Value::Null,
        }
    }

    /// Create an adapter by loading from a file.
    pub fn from_file(path: &Path) -> Result<Self, JsonAdapterError> {
        let content = std::fs::read_to_string(path)?;
        let data: Value = serde_json::from_str(&content)?;
        Ok(Self { data })
    }

    /// Load data from a JSON file.
    pub fn load(&mut self, path: &Path) -> Result<&Value, JsonAdapterError> {
        let content = std::fs::read_to_string(path)?;
        self.data = serde_json::from_str(&content)?;
        Ok(&self.data)
    }

    /// Load data from a JSON string.
    pub fn load_string(&mut self, json_string: &str) -> Result<&Value, JsonAdapterError> {
        self.data = serde_json::from_str(json_string)?;
        Ok(&self.data)
    }

    /// Get the raw data.
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Extract project information.
    pub fn get_project_info(&self) -> ProjectInfo {
        ProjectInfo {
            project: self.get_str("project"),
            project_number: self.get_str("project_number"),
            client: self.get_str("client"),
            author: self.get_str_or("author", "3BM Bouwkunde"),
            report_type: self.get_str("report_type"),
            subtitle: self.get_str("subtitle"),
        }
    }

    /// Extract sections as a JSON array.
    pub fn get_sections(&self) -> Vec<Value> {
        self.data
            .get("sections")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default()
    }

    /// Validate data by attempting typed parsing.
    ///
    /// Uses serde deserialization into `ReportData` as the validation
    /// mechanism (the Rust equivalent of JSON Schema validation).
    ///
    /// Returns a list of validation errors (empty = valid).
    pub fn validate(&self) -> Vec<String> {
        if self.data.is_null() {
            return vec!["Geen data geladen".to_string()];
        }

        // Try typed parsing
        match serde_json::from_value::<ReportData>(self.data.clone()) {
            Ok(_) => Vec::new(),
            Err(e) => vec![e.to_string()],
        }
    }

    /// Basic validation without full schema check.
    pub fn validate_basic(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.data.is_null() {
            errors.push("Geen data geladen".to_string());
            return errors;
        }

        if self.get_str("project").is_empty() {
            errors.push("Verplicht veld ontbreekt: 'project'".to_string());
        }
        if self.get_str("template").is_empty() {
            errors.push("Verplicht veld ontbreekt: 'template'".to_string());
        }

        errors
    }

    /// Parse data into a typed `ReportData`.
    pub fn to_report_data(&self) -> Result<ReportData, JsonAdapterError> {
        let report: ReportData = serde_json::from_value(self.data.clone())?;
        Ok(report)
    }

    // Helpers

    fn get_str(&self, key: &str) -> String {
        self.data
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_str_or(&self, key: &str, default: &str) -> String {
        self.data
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    }
}

impl Default for JsonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Project information extracted from JSON data.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub project: String,
    pub project_number: String,
    pub client: String,
    pub author: String,
    pub report_type: String,
    pub subtitle: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_string() {
        let mut adapter = JsonAdapter::new();
        let json = r#"{"project": "Test", "template": "structural"}"#;
        adapter.load_string(json).unwrap();
        assert_eq!(adapter.get_str("project"), "Test");
    }

    #[test]
    fn test_get_project_info() {
        let mut adapter = JsonAdapter::new();
        adapter
            .load_string(
                r#"{
                "project": "Test Project",
                "project_number": "2026-001",
                "client": "Test Client",
                "report_type": "structural"
            }"#,
            )
            .unwrap();

        let info = adapter.get_project_info();
        assert_eq!(info.project, "Test Project");
        assert_eq!(info.project_number, "2026-001");
        assert_eq!(info.client, "Test Client");
        assert_eq!(info.author, "3BM Bouwkunde"); // default
    }

    #[test]
    fn test_get_sections() {
        let mut adapter = JsonAdapter::new();
        adapter
            .load_string(
                r#"{
                "sections": [
                    {"title": "Section 1"},
                    {"title": "Section 2"}
                ]
            }"#,
            )
            .unwrap();

        let sections = adapter.get_sections();
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_get_sections_missing() {
        let mut adapter = JsonAdapter::new();
        adapter.load_string(r#"{"project": "Test"}"#).unwrap();
        assert!(adapter.get_sections().is_empty());
    }

    #[test]
    fn test_validate_basic_missing_fields() {
        let mut adapter = JsonAdapter::new();
        adapter.load_string(r#"{}"#).unwrap();

        let errors = adapter.validate_basic();
        assert!(errors.len() >= 2);
        assert!(errors.iter().any(|e| e.contains("project")));
        assert!(errors.iter().any(|e| e.contains("template")));
    }

    #[test]
    fn test_validate_basic_valid() {
        let mut adapter = JsonAdapter::new();
        adapter
            .load_string(r#"{"project": "Test", "template": "structural"}"#)
            .unwrap();

        let errors = adapter.validate_basic();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_null_data() {
        let adapter = JsonAdapter::new();
        let errors = adapter.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_from_file_nonexistent() {
        let result = JsonAdapter::from_file(Path::new("/nonexistent/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.json");
        std::fs::write(
            &file,
            r#"{"project": "File Test", "template": "structural"}"#,
        )
        .unwrap();

        let adapter = JsonAdapter::from_file(&file).unwrap();
        assert_eq!(adapter.get_project_info().project, "File Test");
    }
}
