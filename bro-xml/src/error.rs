use crate::BroDocumentType;

#[derive(Debug, thiserror::Error)]
pub enum BroError {
    #[error("invalid XML at {position:?}: {message}")]
    InvalidXml {
        position: Option<u64>,
        message: String,
    },
    #[error("unsupported BRO document root: {root}")]
    UnsupportedDocument { root: String },
    #[error("unsupported {document:?} schema version {version}")]
    UnsupportedSchema {
        document: BroDocumentType,
        version: String,
    },
    #[error("missing required field {path}")]
    MissingField { path: String },
    #[error("invalid value at {path}: {value}")]
    InvalidValue { path: String, value: String },
}
