use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::BroError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// A document family supported by this crate.
pub enum BroDocumentType {
    /// Geotechnical cone penetration test.
    Cpt,
    /// Geotechnical borehole investigation.
    BhrGt,
    /// Geological borehole investigation.
    BhrG,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// A BRO XML schema version.
pub struct SchemaVersion {
    /// Major schema version.
    pub major: u16,
    /// Minor schema version.
    pub minor: u16,
}

impl SchemaVersion {
    /// Creates a schema-version value.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The family and schema version detected from an XML document.
pub struct DetectedDocument {
    /// Detected document family.
    pub document_type: BroDocumentType,
    /// Detected schema version.
    pub schema_version: SchemaVersion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Options shared by all parsing functions.
pub struct ParseOptions {
    /// Whether to retain the complete input XML in the parsed document.
    pub retain_source: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A horizontal position in the coordinate reference system named by [`Self::crs`].
pub struct Position {
    /// Horizontal x coordinate.
    pub x: f64,
    /// Horizontal y coordinate.
    pub y: f64,
    /// Coordinate reference-system identifier found in the source document.
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A vertical offset and its optional datum.
pub struct VerticalPosition {
    /// Vertical offset in metres.
    pub offset: f64,
    /// Vertical datum identifier found in the source document.
    pub datum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Metadata common to all supported BRO document families.
pub struct CommonMetadata {
    /// BRO registration identifier.
    pub bro_id: String,
    /// Schema version used by the source document.
    pub schema_version: SchemaVersion,
    /// Original BRO quality-regime code.
    pub quality_regime: Option<String>,
    /// Accountable-party identifier.
    pub accountable_party: Option<String>,
    /// Registration date.
    pub registration_time: Option<chrono::NaiveDate>,
    /// Date on which the investigation started.
    pub research_start_date: Option<chrono::NaiveDate>,
    /// Date on which the investigation ended.
    pub research_end_date: Option<chrono::NaiveDate>,
    /// Horizontal position, when present.
    pub position: Option<Position>,
    /// Vertical position, when present.
    pub vertical_position: Option<VerticalPosition>,
    /// Unmodelled metadata values, keyed by their XML path.
    pub extensions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
/// A parsed document whose concrete family was detected automatically.
pub enum BroDocument {
    /// A parsed CPT document.
    Cpt(crate::CptDocument),
    /// A parsed geotechnical borehole document.
    BhrGt(crate::BhrGtDocument),
    /// A parsed geological borehole document.
    BhrG(crate::BhrGDocument),
}

/// Detects and parses any supported BRO document family.
///
/// ```
/// let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
/// let document = bro_xml::parse(xml)?;
/// assert!(matches!(document, bro_xml::BroDocument::Cpt(_)));
/// # Ok::<(), bro_xml::BroError>(())
/// ```
pub fn parse(xml: &str) -> Result<BroDocument, BroError> {
    parse_with_options(xml, ParseOptions::default())
}

/// Detects and parses a document with explicit source-retention options.
pub fn parse_with_options(xml: &str, options: ParseOptions) -> Result<BroDocument, BroError> {
    match detect(xml)?.document_type {
        BroDocumentType::Cpt => crate::cpt::parse(xml, options).map(BroDocument::Cpt),
        BroDocumentType::BhrGt => crate::bhr_gt::parse(xml, options).map(BroDocument::BhrGt),
        BroDocumentType::BhrG => crate::bhr_g::parse(xml, options).map(BroDocument::BhrG),
    }
}

/// Detects the family and supported schema version without parsing document fields.
///
/// ```
/// let xml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cpt-minimal.xml"));
/// let detected = bro_xml::detect(xml)?;
/// assert_eq!(detected.document_type, bro_xml::BroDocumentType::Cpt);
/// # Ok::<(), bro_xml::BroError>(())
/// ```
pub fn detect(xml: &str) -> Result<DetectedDocument, BroError> {
    let mut reader = NsReader::from_str(xml);
    let mut root = None;
    let mut detected = None;
    let mut schema_error = None;

    loop {
        match reader.read_resolved_event() {
            Ok((namespace, Event::Start(element) | Event::Empty(element))) => {
                let local_name =
                    String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
                root.get_or_insert_with(|| local_name.clone());

                let Some((document_type, namespace_family, supported_version)) =
                    supported_document(&local_name)
                else {
                    continue;
                };
                let namespace = match namespace {
                    ResolveResult::Bound(namespace) => {
                        String::from_utf8_lossy(namespace.as_ref()).into_owned()
                    }
                    ResolveResult::Unbound => String::new(),
                    ResolveResult::Unknown(prefix) => String::from_utf8_lossy(&prefix).into_owned(),
                };
                let Some(version) = parse_version(&namespace, namespace_family) else {
                    schema_error = Some(BroError::UnsupportedSchema {
                        document: document_type,
                        version: namespace.clone(),
                    });
                    continue;
                };

                if version != supported_version {
                    schema_error = Some(BroError::UnsupportedSchema {
                        document: document_type,
                        version: format!("{}.{}", version.major, version.minor),
                    });
                    continue;
                }

                detected = Some(DetectedDocument {
                    document_type,
                    schema_version: version,
                });
            }
            Ok((_, Event::Eof)) => {
                return match (detected, schema_error) {
                    (_, Some(error)) => Err(error),
                    (Some(detected), None) => Ok(detected),
                    (None, None) => Err(BroError::UnsupportedDocument {
                        root: root.unwrap_or_default(),
                    }),
                };
            }
            Ok(_) => {}
            Err(error) => {
                return Err(BroError::InvalidXml {
                    position: Some(reader.buffer_position()),
                    message: error.to_string(),
                });
            }
        }
    }
}

fn supported_document(local_name: &str) -> Option<(BroDocumentType, &'static str, SchemaVersion)> {
    match local_name {
        "CPT_O" => Some((BroDocumentType::Cpt, "dscpt", SchemaVersion::new(1, 1))),
        "BHR_GT_O" => Some((BroDocumentType::BhrGt, "dsbhr-gt", SchemaVersion::new(2, 1))),
        "BHR_G_O" => Some((BroDocumentType::BhrG, "dsbhrg", SchemaVersion::new(3, 1))),
        _ => None,
    }
}

fn parse_version(namespace: &str, expected_family: &str) -> Option<SchemaVersion> {
    let mut segments = namespace.trim_end_matches('/').rsplit('/');
    let version = segments.next()?;
    let family = segments.next()?;
    if family != expected_family {
        return None;
    }

    let (major, minor) = version.split_once('.')?;
    Some(SchemaVersion::new(major.parse().ok()?, minor.parse().ok()?))
}
