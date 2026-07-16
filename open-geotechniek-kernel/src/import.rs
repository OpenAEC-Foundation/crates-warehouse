use crate::{GeotechnicalObject, KernelError};

pub(crate) fn parse_bro(xml: &str, source_file: &str) -> Result<GeotechnicalObject, KernelError> {
    match bro_xml::parse(xml)? {
        bro_xml::BroDocument::BhrGt(document) => Ok(GeotechnicalObject::BhrGt(document)),
        bro_xml::BroDocument::BhrG(document) => Ok(GeotechnicalObject::BhrG(document)),
        bro_xml::BroDocument::Cpt(document) => Ok(GeotechnicalObject::Cpt(crate::cpt::from_bro(
            document,
            source_file,
        ))),
    }
}

pub(crate) fn parse_cpt(content: &str, source_file: &str) -> Result<cpt_core::Cpt, KernelError> {
    let mut cpt = if source_file.to_ascii_lowercase().ends_with(".ifcgeo") {
        cpt_core::write::read_ifcgeo(content)?
    } else {
        cpt_core::parse_auto(content)?
    };
    cpt.metadata.source_file = source_file.to_owned();
    Ok(cpt)
}
