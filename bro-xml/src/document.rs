use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};

use crate::BroError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BroDocumentType {
    Cpt,
    BhrGt,
    BhrG,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}

impl SchemaVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetectedDocument {
    pub document_type: BroDocumentType,
    pub schema_version: SchemaVersion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseOptions {
    pub retain_source: bool,
}

pub fn detect(xml: &str) -> Result<DetectedDocument, BroError> {
    let mut reader = NsReader::from_str(xml);
    let mut root = None;

    loop {
        match reader.read_resolved_event() {
            Ok((namespace, Event::Start(element) | Event::Empty(element))) => {
                let local_name =
                    String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
                root.get_or_insert_with(|| local_name.clone());

                let Some((document_type, supported_version)) = supported_document(&local_name)
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
                let version =
                    parse_version(&namespace).ok_or_else(|| BroError::UnsupportedSchema {
                        document: document_type,
                        version: namespace.clone(),
                    })?;

                if version != supported_version {
                    return Err(BroError::UnsupportedSchema {
                        document: document_type,
                        version: format!("{}.{}", version.major, version.minor),
                    });
                }

                return Ok(DetectedDocument {
                    document_type,
                    schema_version: version,
                });
            }
            Ok((_, Event::Eof)) => {
                return Err(BroError::UnsupportedDocument {
                    root: root.unwrap_or_default(),
                });
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

fn supported_document(local_name: &str) -> Option<(BroDocumentType, SchemaVersion)> {
    match local_name {
        "CPT_O" => Some((BroDocumentType::Cpt, SchemaVersion::new(1, 1))),
        "BHR_GT_O" => Some((BroDocumentType::BhrGt, SchemaVersion::new(2, 1))),
        "BHR_G_O" => Some((BroDocumentType::BhrG, SchemaVersion::new(3, 1))),
        _ => None,
    }
}

fn parse_version(namespace: &str) -> Option<SchemaVersion> {
    let suffix = namespace.trim_end_matches('/').rsplit('/').next()?;
    let (major, minor) = suffix.split_once('.')?;
    Some(SchemaVersion::new(major.parse().ok()?, minor.parse().ok()?))
}
