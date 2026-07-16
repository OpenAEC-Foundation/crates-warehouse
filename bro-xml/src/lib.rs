//! Typed, network-free parsing of BRO CPT, BHR-GT and BHR-G XML documents.
//!
//! Use [`detect`] or [`parse`] when a caller does not know the document type in
//! advance. The typed parser functions return a document-specific structure.

#![deny(missing_docs)]

mod bhr_g;
mod bhr_gt;
mod cpt;
mod document;
mod error;
mod reference_codes;
mod xml;

pub use bhr_g::{BhrGDocument, GeologicalInterval};
pub use bhr_gt::{BhrGtDocument, GeotechnicalInterval, SecondaryAttribute};
pub use cpt::{CptDocument, CptMeasurement};
pub use document::{
    detect, parse, parse_with_options, BroDocument, BroDocumentType, CommonMetadata,
    DetectedDocument, ParseOptions, Position, SchemaVersion, VerticalPosition,
};
pub use error::BroError;
pub use reference_codes::{describe_reference_code, ReferenceCodeSet};

/// Parses a CPT document.
///
/// ```
/// let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
/// let cpt = bro_xml::parse_cpt(xml)?;
/// assert!(!cpt.measurements.is_empty());
/// # Ok::<(), bro_xml::BroError>(())
/// ```
pub fn parse_cpt(xml: &str) -> Result<CptDocument, BroError> {
    parse_cpt_with_options(xml, ParseOptions::default())
}

/// Parses a CPT document with explicit source-retention options.
pub fn parse_cpt_with_options(xml: &str, options: ParseOptions) -> Result<CptDocument, BroError> {
    crate::cpt::parse(xml, options)
}

/// Parses a geotechnical borehole document.
///
/// ```
/// let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bhr-gt-minimal.xml"));
/// let borehole = bro_xml::parse_bhr_gt(xml)?;
/// assert_eq!(borehole.intervals.len(), 2);
/// # Ok::<(), bro_xml::BroError>(())
/// ```
pub fn parse_bhr_gt(xml: &str) -> Result<BhrGtDocument, BroError> {
    parse_bhr_gt_with_options(xml, ParseOptions::default())
}

/// Parses a geotechnical borehole document with explicit source-retention options.
pub fn parse_bhr_gt_with_options(
    xml: &str,
    options: ParseOptions,
) -> Result<BhrGtDocument, BroError> {
    crate::bhr_gt::parse(xml, options)
}

/// Parses a geological borehole document.
///
/// ```
/// let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bhr-g-minimal.xml"));
/// let borehole = bro_xml::parse_bhr_g(xml)?;
/// assert_eq!(borehole.intervals.len(), 2);
/// # Ok::<(), bro_xml::BroError>(())
/// ```
pub fn parse_bhr_g(xml: &str) -> Result<BhrGDocument, BroError> {
    parse_bhr_g_with_options(xml, ParseOptions::default())
}

/// Parses a geological borehole document with explicit source-retention options.
pub fn parse_bhr_g_with_options(
    xml: &str,
    options: ParseOptions,
) -> Result<BhrGDocument, BroError> {
    crate::bhr_g::parse(xml, options)
}
