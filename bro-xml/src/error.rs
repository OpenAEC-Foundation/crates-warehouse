use crate::BroDocumentType;

#[derive(Debug, thiserror::Error)]
/// An error encountered while detecting or parsing a BRO XML document.
pub enum BroError {
    /// The input is not well-formed XML.
    #[error("invalid XML at {position:?}: {message}")]
    InvalidXml {
        /// Byte position reported by the XML reader, when available.
        position: Option<u64>,
        /// XML-reader diagnostic.
        message: String,
    },
    /// The root element does not identify a supported BRO document family.
    #[error("unsupported BRO document root: {root}")]
    UnsupportedDocument {
        /// Root element local name.
        root: String,
    },
    /// The document family is known but its schema version is unsupported.
    #[error("unsupported {document:?} schema version {version}")]
    UnsupportedSchema {
        /// Detected document family.
        document: BroDocumentType,
        /// Unsupported version or namespace string.
        version: String,
    },
    /// A typed parser received a different supported document family.
    #[error("expected {expected:?} document, found {found:?}")]
    UnexpectedDocumentType {
        /// Document family accepted by the called parser.
        expected: BroDocumentType,
        /// Document family detected in the input.
        found: BroDocumentType,
    },
    /// A required XML field is absent.
    #[error("missing required field {path}")]
    MissingField {
        /// XML path of the missing field.
        path: String,
    },
    /// A field contains a value that cannot be interpreted safely.
    #[error("invalid value at {path}: {value}")]
    InvalidValue {
        /// XML path of the invalid field.
        path: String,
        /// Original invalid value.
        value: String,
    },
}
