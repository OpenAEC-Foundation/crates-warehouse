//! Core domain types — see Task 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cpt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementPoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position;
