mod bhr_g;
mod bhr_gt;
mod cpt;
mod document;
mod error;
mod xml;

pub use bhr_g::{BhrGDocument, GeologicalInterval};
pub use bhr_gt::{BhrGtDocument, GeotechnicalInterval, SecondaryAttribute};
pub use cpt::{CptDocument, CptMeasurement};
pub use document::{
    detect, parse, parse_with_options, BroDocument, BroDocumentType, CommonMetadata,
    DetectedDocument, ParseOptions, Position, SchemaVersion, VerticalPosition,
};
pub use error::BroError;

pub fn parse_cpt(xml: &str) -> Result<CptDocument, BroError> {
    parse_cpt_with_options(xml, ParseOptions::default())
}

pub fn parse_cpt_with_options(xml: &str, options: ParseOptions) -> Result<CptDocument, BroError> {
    crate::cpt::parse(xml, options)
}

pub fn parse_bhr_gt(xml: &str) -> Result<BhrGtDocument, BroError> {
    parse_bhr_gt_with_options(xml, ParseOptions::default())
}

pub fn parse_bhr_gt_with_options(
    xml: &str,
    options: ParseOptions,
) -> Result<BhrGtDocument, BroError> {
    crate::bhr_gt::parse(xml, options)
}

pub fn parse_bhr_g(xml: &str) -> Result<BhrGDocument, BroError> {
    parse_bhr_g_with_options(xml, ParseOptions::default())
}

pub fn parse_bhr_g_with_options(
    xml: &str,
    options: ParseOptions,
) -> Result<BhrGDocument, BroError> {
    crate::bhr_g::parse(xml, options)
}
