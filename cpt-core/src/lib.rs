//! CPT (Cone Penetration Test) domain library.
//!
//! Parses GEF and BRO-XML CPT files, classifies measurement points using
//! Robertson 1990 SBT, detects soil layers, and builds standardized reports.

pub mod bro;
pub mod coords;
pub mod domain;
pub mod error;
pub mod gef;
pub mod layers;
pub mod robertson;

pub use domain::{Cpt, MeasurementPoint, Metadata, Position};
pub use error::CptError;
pub use layers::{detect_layers, Layer};
