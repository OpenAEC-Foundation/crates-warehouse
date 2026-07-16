use serde::{Deserialize, Serialize};

use crate::{detect, xml, BroError, CommonMetadata, ParseOptions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BhrGDocument {
    pub common: CommonMetadata,
    pub source_xml: Option<String>,
}

pub(crate) fn parse(xml_source: &str, options: ParseOptions) -> Result<BhrGDocument, BroError> {
    let detected = detect(xml_source)?;
    let collected = xml::collect(xml_source)?;
    Ok(BhrGDocument {
        common: xml::common_metadata(&collected, detected.schema_version)?,
        source_xml: options.retain_source.then(|| xml_source.to_owned()),
    })
}
