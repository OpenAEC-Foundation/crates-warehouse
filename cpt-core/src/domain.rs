//! Core domain types — `Cpt` and friends.
//!
//! All types derive `Serialize + Deserialize` so they cross the Tauri IPC
//! boundary directly. Numeric fields use `f64` to match GEF/BRO source precision.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cpt {
    pub id: String,
    pub metadata: Metadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub points: Vec<MeasurementPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equipment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ground_level_nap: Option<f64>,
    pub source_file: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x_rd: f64,
    pub y_rd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_nap: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeasurementPoint {
    pub depth: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_nap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rf: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inclination: Option<f64>,
}
