//! `.ifcgis` — Open GEO Studio project file format.
//!
//! A JSON container in IFCX-flavoured style that bundles project metadata
//! plus all CPTs of a project into a single file. Follows the OpenAEC
//! ecosystem's preference for IFCX (IFC5 alpha) — flat object lists, stable
//! types, mergeable in git.
//!
//! For v1 we don't depend on the full IFCX schema (still in flux); we use
//! OpenGEO-prefixed types (`OpenGeoProject`, `OpenGeoCpt`) and the IFCX
//! conventions for header + object listing. A future `cpt-ifcx` crate can
//! map this into strict IFCX once that schema stabilises.
//!
//! File extension: `.ifcgis`
//! Media type: `application/x.ifcgis+json`

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::Cpt;
use crate::error::CptError;

const SCHEMA_VERSION: &str = "ifcgis-0.1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub header: Header,
    pub project: ProjectInfo,
    pub cpts: Vec<Cpt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub schema: String,
    pub originating_system: String,
    pub timestamp: String, // RFC3339
}

impl Header {
    pub fn new(originating_system: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA_VERSION.into(),
            originating_system: originating_system.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(rename = "type", default = "default_project_type")]
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub client: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub location: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub project_number: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub author: String,
    pub date: NaiveDate,
}

fn default_project_type() -> String {
    "OpenGeoProject".to_string()
}

/// Serialise a project (metadata + CPTs) to a pretty-printed `.ifcgis` JSON string.
pub fn save(project: ProjectInfo, cpts: Vec<Cpt>) -> Result<String, CptError> {
    let file = ProjectFile {
        header: Header::new("Open GEO Studio"),
        project,
        cpts,
    };
    serde_json::to_string_pretty(&file)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis serialize: {e}")))
}

/// Parse a `.ifcgis` JSON file into project metadata and CPTs.
/// Tolerates older schema versions as long as the data shape matches.
pub fn load(text: &str) -> Result<ProjectFile, CptError> {
    let file: ProjectFile = serde_json::from_str(text)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis parse: {e}")))?;
    if !file.header.schema.starts_with("ifcgis-") {
        return Err(CptError::InvalidGef(format!(
            "unrecognized schema '{}' (expected ifcgis-*)",
            file.header.schema
        )));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Metadata, MeasurementPoint, Position};

    fn sample_cpt(id: &str) -> Cpt {
        Cpt {
            id: id.to_string(),
            metadata: Metadata { source_file: format!("{id}.gef"), ..Default::default() },
            position: Some(Position { x_rd: 100_000.0, y_rd: 400_000.0, z_nap: Some(2.5) }),
            points: vec![
                MeasurementPoint { depth: 0.02, depth_nap: Some(2.48), qc: Some(1.5), fs: Some(0.015), rf: Some(1.0), u2: None, inclination: None },
                MeasurementPoint { depth: 0.04, depth_nap: Some(2.46), qc: Some(1.6), fs: Some(0.016), rf: Some(1.0), u2: None, inclination: None },
            ],
        }
    }

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            kind: default_project_type(),
            title: "Test project".into(),
            client: "ACME bv".into(),
            location: "Amsterdam".into(),
            project_number: "2026-001".into(),
            author: "Open GEO Studio".into(),
            date: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        }
    }

    #[test]
    fn round_trip_empty_project() {
        let json = save(sample_project(), vec![]).unwrap();
        assert!(json.contains("\"schema\": \"ifcgis-0.1\""));
        let back = load(&json).unwrap();
        assert_eq!(back.project.title, "Test project");
        assert_eq!(back.cpts.len(), 0);
    }

    #[test]
    fn round_trip_with_cpts() {
        let json = save(sample_project(), vec![sample_cpt("S01"), sample_cpt("S02")]).unwrap();
        let back = load(&json).unwrap();
        assert_eq!(back.cpts.len(), 2);
        assert_eq!(back.cpts[0].id, "S01");
        assert_eq!(back.cpts[0].points.len(), 2);
        assert_eq!(back.cpts[0].position.unwrap().x_rd, 100_000.0);
    }

    #[test]
    fn rejects_unknown_schema() {
        let bad = r#"{"header":{"schema":"openfoo-1","originating_system":"X","timestamp":"2026-01-01T00:00:00Z"},"project":{"type":"OpenGeoProject","title":"T","date":"2026-01-01"},"cpts":[]}"#;
        let err = load(bad).err().unwrap();
        assert!(format!("{err}").contains("unrecognized schema"));
    }
}
