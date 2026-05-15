//! Template loader — YAML report template loading and parsing.
//!
//! Port of `core/template_loader.py`. Loads YAML templates with multi-directory
//! support (tenant-specific + package defaults).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Parsed template configuration (legacy format).
///
/// This is the format used by `engine.rs` and the older ReportLab-based renderer.
/// For the v2 template_engine format with page_types, see `TemplateConfigV2`
/// in `template_config.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub name: String,
    #[serde(default)]
    pub report_type: String,
    #[serde(default)]
    pub tenant: String,
    #[serde(default = "default_a4")]
    pub format: String,
    #[serde(default = "default_portrait")]
    pub orientation: String,
    #[serde(default = "default_margins")]
    pub margins: HashMap<String, f64>,
    #[serde(default)]
    pub header: serde_yaml::Value,
    #[serde(default)]
    pub footer: serde_yaml::Value,
    #[serde(default)]
    pub cover: serde_yaml::Value,
    #[serde(default)]
    pub colofon: serde_yaml::Value,
    #[serde(default)]
    pub toc: serde_yaml::Value,
    #[serde(default)]
    pub backcover: serde_yaml::Value,
    #[serde(default)]
    pub structure: Vec<String>,
    /// Full unparsed YAML data (preserved for scaffold generation).
    #[serde(skip)]
    pub raw: serde_yaml::Value,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            report_type: String::new(),
            tenant: String::new(),
            format: "A4".to_string(),
            orientation: "portrait".to_string(),
            margins: default_margins(),
            header: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            footer: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            cover: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            colofon: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            toc: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            backcover: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            structure: Vec::new(),
            raw: serde_yaml::Value::Null,
        }
    }
}

/// Template loading error.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("Template '{0}' niet gevonden")]
    NotFound(String),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// YAML report template loader.
///
/// Loads and parses templates with multi-directory support (tenant + package).
pub struct TemplateLoader {
    templates_dirs: Vec<PathBuf>,
}

impl TemplateLoader {
    /// Create a new loader with a single templates directory.
    pub fn new(templates_dir: PathBuf) -> Self {
        Self {
            templates_dirs: vec![templates_dir],
        }
    }

    /// Create a new loader with multiple directories (tenant-first).
    pub fn with_dirs(templates_dirs: Vec<PathBuf>) -> Self {
        Self { templates_dirs }
    }

    /// The primary templates directory (first in list).
    pub fn templates_dir(&self) -> &Path {
        &self.templates_dirs[0]
    }

    /// Load a template by name.
    ///
    /// Searches all configured directories (tenant first, then package defaults).
    pub fn load(&self, name: &str) -> Result<TemplateConfig, TemplateError> {
        let path = self.resolve_path(name);
        if !path.exists() {
            return Err(TemplateError::NotFound(name.to_string()));
        }

        let content = std::fs::read_to_string(&path)?;
        let raw: serde_yaml::Value = serde_yaml::from_str(&content)?;

        let data = if let serde_yaml::Value::Mapping(ref map) = raw {
            map
        } else {
            return Ok(TemplateConfig {
                name: name.to_string(),
                raw,
                ..Default::default()
            });
        };

        let get_str = |key: &str, default: &str| -> String {
            data.get(serde_yaml::Value::String(key.to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string()
        };

        let margins = data
            .get(serde_yaml::Value::String("margins".to_string()))
            .and_then(|v| serde_yaml::from_value(v.clone()).ok())
            .unwrap_or_else(default_margins);

        let structure = data
            .get(serde_yaml::Value::String("structure".to_string()))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let get_value = |key: &str| -> serde_yaml::Value {
            data.get(serde_yaml::Value::String(key.to_string()))
                .cloned()
                .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        };

        Ok(TemplateConfig {
            name: name.to_string(),
            report_type: get_str("report_type", ""),
            tenant: get_str("tenant", ""),
            format: get_str("format", "A4"),
            orientation: get_str("orientation", "portrait"),
            margins,
            header: get_value("header"),
            footer: get_value("footer"),
            cover: get_value("cover"),
            colofon: get_value("colofon"),
            toc: get_value("toc"),
            backcover: get_value("backcover"),
            structure,
            raw,
        })
    }

    /// List all available templates (merged from all directories).
    ///
    /// Tenant templates come first; duplicate names are skipped.
    pub fn list_templates(&self) -> Vec<TemplateInfo> {
        let mut seen = std::collections::HashSet::new();
        let mut templates = Vec::new();

        for dir in &self.templates_dirs {
            if !dir.exists() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "yaml"))
                .collect();
            paths.sort();

            for path in paths {
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !seen.insert(stem.to_string()) {
                    continue;
                }
                let report_type = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| serde_yaml::from_str::<serde_yaml::Value>(&content).ok())
                    .and_then(|v| {
                        v.get("report_type")
                            .and_then(|rt| rt.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();

                templates.push(TemplateInfo {
                    name: stem.to_string(),
                    report_type,
                });
            }
        }

        templates
    }

    /// Generate an empty JSON scaffold from a template.
    ///
    /// The scaffold contains all metadata fields with defaults from the template,
    /// cover/colofon/toc/backcover configuration, and an empty sections array.
    /// Suitable as a starting point for the frontend.
    pub fn to_scaffold(&self, name: &str) -> Result<serde_json::Value, TemplateError> {
        let config = self.load(name)?;
        let today = today_iso();

        let default_disclaimer =
            "Dit rapport is opgesteld door 3BM Bouwkunde en is uitsluitend \
             bedoeld voor de opdrachtgever. Verspreiding aan derden is niet \
             toegestaan zonder schriftelijke toestemming.";

        // Colofon
        let colofon_enabled = yaml_bool(&config.colofon, "enabled", true);
        let disclaimer = yaml_str(&config.colofon, "disclaimer")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_disclaimer.to_string());

        let colofon = serde_json::json!({
            "enabled": colofon_enabled,
            "opdrachtgever_naam": "",
            "opdrachtgever_contact": "",
            "opdrachtgever_adres": "",
            "adviseur_bedrijf": "",
            "adviseur_naam": "",
            "adviseur_email": "",
            "adviseur_telefoon": "",
            "adviseur_functie": "",
            "adviseur_registratie": "",
            "normen": "",
            "documentgegevens": "",
            "datum": today,
            "fase": "",
            "status_colofon": "CONCEPT",
            "kenmerk": "",
            "extra_fields": {},
            "revision_history": [{
                "version": "0.1",
                "date": today,
                "author": "",
                "description": "Eerste opzet"
            }],
            "disclaimer": disclaimer
        });

        // TOC
        let toc = serde_json::json!({
            "enabled": yaml_bool(&config.toc, "enabled", true),
            "title": yaml_str(&config.toc, "title").unwrap_or_else(|| "Inhoudsopgave".to_string()),
            "max_depth": yaml_int(&config.toc, "max_depth", 3),
        });

        // Cover
        let cover = serde_json::json!({
            "subtitle": yaml_str(&config.cover, "subtitle_hint").unwrap_or_default(),
        });

        // Backcover
        let backcover = serde_json::json!({
            "enabled": yaml_bool(&config.backcover, "enabled", true),
        });

        let brand = if config.tenant.is_empty() {
            "3bm_cooperatie"
        } else {
            &config.tenant
        };

        let scaffold = serde_json::json!({
            "template": name,
            "format": config.format,
            "orientation": config.orientation,
            "project": "",
            "project_number": "",
            "client": "",
            "author": "3BM Bouwkunde",
            "brand": brand,
            "date": today,
            "version": "1.0",
            "status": "CONCEPT",
            "report_type": config.report_type,
            "cover": cover,
            "colofon": colofon,
            "toc": toc,
            "sections": [],
            "backcover": backcover,
            "metadata": {},
        });

        Ok(scaffold)
    }

    /// Resolve a template name to a file path.
    ///
    /// Searches all directories (tenant first). If not found, returns
    /// the path in the first directory (for error messages).
    fn resolve_path(&self, name: &str) -> PathBuf {
        let filename = if name.ends_with(".yaml") {
            name.to_string()
        } else {
            format!("{name}.yaml")
        };

        for dir in &self.templates_dirs {
            let candidate = dir.join(&filename);
            if candidate.exists() {
                return candidate;
            }
        }

        // Not found — return path in first dir for error message
        self.templates_dirs[0].join(&filename)
    }
}

/// Template info returned by `list_templates`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    pub report_type: String,
}

// ── Helpers ────────────────────────────────────────────────────────────

fn default_a4() -> String {
    "A4".to_string()
}

fn default_portrait() -> String {
    "portrait".to_string()
}

fn default_margins() -> HashMap<String, f64> {
    let mut m = HashMap::new();
    m.insert("top".to_string(), 25.0);
    m.insert("bottom".to_string(), 20.0);
    m.insert("left".to_string(), 20.0);
    m.insert("right".to_string(), 15.0);
    m
}

fn today_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn yaml_str(value: &serde_yaml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
}

fn yaml_bool(value: &serde_yaml::Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn yaml_int(value: &serde_yaml::Value, key: &str, default: i64) -> i64 {
    value.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_adds_yaml() {
        let loader = TemplateLoader::new(PathBuf::from("/tmp/templates"));
        let path = loader.resolve_path("structural_report");
        assert!(path.to_str().unwrap().ends_with("structural_report.yaml"));
    }

    #[test]
    fn test_resolve_path_keeps_yaml() {
        let loader = TemplateLoader::new(PathBuf::from("/tmp/templates"));
        let path = loader.resolve_path("test.yaml");
        assert!(path.to_str().unwrap().ends_with("test.yaml"));
        // Should not double-suffix
        assert!(!path.to_str().unwrap().ends_with("test.yaml.yaml"));
    }

    #[test]
    fn test_list_templates_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loader = TemplateLoader::new(dir.path().to_path_buf());
        let list = loader.list_templates();
        assert!(list.is_empty());
    }

    #[test]
    fn test_load_template() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
report_type: structural
format: A4
orientation: portrait
cover:
  subtitle_hint: "Constructieve berekening"
toc:
  enabled: true
  max_depth: 2
structure:
  - uitgangspunten
  - berekening
  - conclusie
"#;
        std::fs::write(dir.path().join("test.yaml"), yaml).unwrap();

        let loader = TemplateLoader::new(dir.path().to_path_buf());
        let config = loader.load("test").unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.report_type, "structural");
        assert_eq!(config.format, "A4");
        assert_eq!(config.structure.len(), 3);
    }

    #[test]
    fn test_to_scaffold() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
report_type: structural
tenant: 3bm_cooperatie
format: A4
toc:
  enabled: true
  title: "Inhoud"
  max_depth: 2
cover:
  subtitle_hint: "Constructief rapport"
colofon:
  enabled: true
backcover:
  enabled: true
"#;
        std::fs::write(dir.path().join("struct.yaml"), yaml).unwrap();

        let loader = TemplateLoader::new(dir.path().to_path_buf());
        let scaffold = loader.to_scaffold("struct").unwrap();

        assert_eq!(scaffold["template"], "struct");
        assert_eq!(scaffold["report_type"], "structural");
        assert_eq!(scaffold["brand"], "3bm_cooperatie");
        assert_eq!(scaffold["toc"]["title"], "Inhoud");
        assert_eq!(scaffold["toc"]["max_depth"], 2);
        assert_eq!(scaffold["cover"]["subtitle"], "Constructief rapport");
        assert!(scaffold["sections"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_template_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let loader = TemplateLoader::new(dir.path().to_path_buf());
        let result = loader.load("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TemplateError::NotFound(_)));
    }

    #[test]
    fn test_multi_dir_priority() {
        let tenant_dir = tempfile::tempdir().unwrap();
        let package_dir = tempfile::tempdir().unwrap();

        // Same template name in both dirs — different report_type
        std::fs::write(
            tenant_dir.path().join("shared.yaml"),
            "report_type: tenant_version",
        )
        .unwrap();
        std::fs::write(
            package_dir.path().join("shared.yaml"),
            "report_type: package_version",
        )
        .unwrap();

        let loader = TemplateLoader::with_dirs(vec![
            tenant_dir.path().to_path_buf(),
            package_dir.path().to_path_buf(),
        ]);

        let config = loader.load("shared").unwrap();
        // Tenant wins
        assert_eq!(config.report_type, "tenant_version");
    }
}
