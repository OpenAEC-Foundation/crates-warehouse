//! Error type for the CPT library.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CptError {
    #[error("invalid GEF file: {0}")]
    InvalidGef(String),

    #[error("invalid BRO XML file: {0}")]
    InvalidBro(String),

    #[error("unknown CPT format (expected GEF header '#GEF' or XML root)")]
    UnknownFormat,

    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("number parse error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_displays_message() {
        let e = CptError::InvalidGef("missing #EOH".to_string());
        assert_eq!(format!("{}", e), "invalid GEF file: missing #EOH");
    }

    #[test]
    fn error_unknown_format_message() {
        let e = CptError::UnknownFormat;
        assert!(format!("{}", e).contains("unknown CPT format"));
    }
}
