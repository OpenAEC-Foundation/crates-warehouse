//! CPT (Cone Penetration Test) domain library.
//!
//! Parses GEF and BRO-XML CPT files, classifies measurement points using
//! Robertson 1990 SBT, detects soil layers, and builds standardized reports.

pub mod bro;
pub mod coords;
pub mod domain;
pub mod error;
pub mod gef;
pub mod ifc;
pub mod ifcgis;
pub mod layers;
pub mod pdf;
pub mod plot;
pub mod report;
pub mod robertson;
pub mod write;

pub use domain::{Cpt, MeasurementPoint, Metadata, Position};
pub use error::CptError;
pub use ifc::{write_ifc4x3, write_ifcx};
pub use layers::{detect_layers, Layer};
pub use pdf::generate_single_cpt_pdf_bytes;
pub use plot::render_cpt_svg;
pub use report::{build as build_report, ProjectMeta};

pub use gef::parse as parse_gef;
pub use bro::parse as parse_bro;

/// Detect format from the first non-whitespace bytes and dispatch.
/// - GEF files start with `#GEF` (or sometimes `#GEFID`)
/// - BRO XML starts with `<?xml` or `<` (ignoring leading whitespace)
pub fn parse_auto(content: &str) -> Result<Cpt, CptError> {
    let trimmed = content.trim_start();
    if trimmed.starts_with("#GEF") || trimmed.starts_with("#GEFID") {
        return parse_gef(content);
    }
    if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
        return parse_bro(content);
    }
    Err(CptError::UnknownFormat)
}
