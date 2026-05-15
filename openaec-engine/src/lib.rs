//! OpenAEC Engine — V3 TemplateEngine for coordinate-based PDF rendering.
//!
//! Generates pixel-perfect PDF reports from YAML page_type templates,
//! brand configuration, and JSON data. No Python or AGPL dependencies.

pub mod error;
pub mod pdf_backend;
pub mod font_engine;
pub mod text;
pub mod data_bind;
pub mod zone_renderer;
pub mod flow_layout;
pub mod engine;
pub mod special_pages;

pub use engine::{Engine, EngineConfig};
pub use error::{EngineError, Result};
