use serde::{Deserialize, Serialize};

use crate::{detect, xml, BroError, CommonMetadata, ParseOptions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CptDocument {
    pub common: CommonMetadata,
    pub measurements: Vec<CptMeasurement>,
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CptMeasurement {}

pub(crate) fn parse(xml_source: &str, options: ParseOptions) -> Result<CptDocument, BroError> {
    let detected = detect(xml_source)?;
    let collected = xml::collect(xml_source)?;
    Ok(CptDocument {
        common: xml::common_metadata(&collected, detected.schema_version)?,
        measurements: Vec::new(),
        source_xml: options.retain_source.then(|| xml_source.to_owned()),
    })
}
