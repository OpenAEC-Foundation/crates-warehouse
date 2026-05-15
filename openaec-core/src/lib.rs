// Existing modules
pub mod schema;
pub mod brand;
pub mod tenant;
pub mod font_manager;
pub mod block_renderer;
pub mod special_pages;
pub mod engine;

// Phase 2 modules
pub mod document;
pub mod template_config;
pub mod styles;
pub mod template_loader;
pub mod data_transform;
pub mod stationery;
pub mod toc;
pub mod kadaster;
pub mod json_adapter;
pub mod report_types;

// Re-exports — existing
pub use schema::ReportData;
pub use brand::{BrandConfig, BrandLoader};
pub use tenant::TenantConfig;
pub use font_manager::FontManager;
pub use engine::{generate_pdf, generate_pdf_bytes, EngineError};

// Re-exports — Phase 2
pub use document::{DocumentConfig, Margins, PageFormatMm};
pub use styles::StyleSet;
pub use template_loader::TemplateLoader;
pub use stationery::{StationeryResolver, StationerySpec};
pub use toc::TocBuilder;
pub use kadaster::KadasterClient;
pub use json_adapter::JsonAdapter;
pub use report_types::{get_builder, ReportBuilder};
