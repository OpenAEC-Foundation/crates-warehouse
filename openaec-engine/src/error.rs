//! Error types for openaec-engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Font error: {0}")]
    Font(String),

    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("Stationery error: {0}")]
    Stationery(String),

    #[error("Image error: {0}")]
    Image(String),

    #[error("Template error: {0}")]
    Template(String),

    #[error("Data binding error: {0}")]
    DataBind(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, EngineError>;
