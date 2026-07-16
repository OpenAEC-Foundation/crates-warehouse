use serde::{Deserialize, Serialize};

use crate::{detect, xml, BroError, CommonMetadata, ParseOptions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BhrGtDocument {
    pub common: CommonMetadata,
    pub source_xml: Option<String>,
}

pub(crate) fn parse(xml_source: &str, options: ParseOptions) -> Result<BhrGtDocument, BroError> {
    let detected = detect(xml_source)?;
    let collected = xml::collect(xml_source)?;
    Ok(BhrGtDocument {
        common: xml::common_metadata(&collected, detected.schema_version)?,
        source_xml: options.retain_source.then(|| xml_source.to_owned()),
    })
}
