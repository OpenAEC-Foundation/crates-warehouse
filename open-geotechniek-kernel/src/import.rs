use crate::{GeotechnicalObject, KernelError};

pub(crate) fn parse_bro(xml: &str, source_file: &str) -> Result<GeotechnicalObject, KernelError> {
    let options = bro_xml::ParseOptions {
        retain_source: true,
    };
    match bro_xml::parse_with_options(xml, options)? {
        bro_xml::BroDocument::BhrGt(mut document) => {
            document
                .common
                .extensions
                .insert("openGeo/sourceFile".to_owned(), source_file.to_owned());
            Ok(GeotechnicalObject::BhrGt(document))
        }
        bro_xml::BroDocument::BhrG(mut document) => {
            document
                .common
                .extensions
                .insert("openGeo/sourceFile".to_owned(), source_file.to_owned());
            Ok(GeotechnicalObject::BhrG(document))
        }
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
