//! Project-level operations for geotechnical content.
//!
//! The kernel owns no filesystem or network access. Callers pass document
//! content together with a source label and receive in-memory domain objects.
//!
//! ```
//! use open_geotechniek_kernel::{GeotechnicalProject, ProjectMetadata};
//!
//! let mut project = GeotechnicalProject::new(ProjectMetadata {
//!     title: "Example".to_owned(),
//!     ..ProjectMetadata::default()
//! });
//! let ifcgeo = r#"{
//!     "id": "CPT-1",
//!     "metadata": { "source_file": "original.ifcgeo" },
//!     "position": null,
//!     "points": []
//! }"#;
//! project.import_cpt(ifcgeo, "CPT-1.ifcgeo")?;
//! assert_eq!(project.get("CPT-1")?.id(), "CPT-1");
//! assert!(project.detect_cpt_layers("CPT-1")?.is_empty());
//! # Ok::<(), open_geotechniek_kernel::KernelError>(())
//! ```

#![deny(missing_docs)]

mod cpt;
mod error;
mod import;
mod object;
mod project;

pub use error::KernelError;
pub use object::{GeotechnicalObject, ObjectKind};
pub use project::{DuplicatePolicy, GeotechnicalProject, ProjectMetadata};
