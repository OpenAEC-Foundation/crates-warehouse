/// Errors exposed by the geotechnical project boundary.
#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    /// A BRO document could not be parsed.
    #[error(transparent)]
    Bro(#[from] bro_xml::BroError),
    /// A CPT document could not be parsed.
    #[error(transparent)]
    Cpt(#[from] cpt_core::CptError),
    /// An object with the supplied identifier already exists.
    #[error("duplicate geotechnical object {id}")]
    DuplicateObject {
        /// Conflicting stable object identifier.
        id: String,
    },
    /// No object with the supplied identifier exists.
    #[error("geotechnical object not found: {id}")]
    ObjectNotFound {
        /// Requested stable object identifier.
        id: String,
    },
    /// Project content violates a project invariant.
    #[error("invalid project: {message}")]
    InvalidProject {
        /// Description of the violated project invariant.
        message: String,
    },
    /// A conversion between supported domain models failed.
    #[error("conversion failed: {message}")]
    Conversion {
        /// Description of the conversion failure.
        message: String,
    },
    /// Content could not be exported.
    #[error("export failed: {message}")]
    Export {
        /// Description of the export failure.
        message: String,
    },
}
