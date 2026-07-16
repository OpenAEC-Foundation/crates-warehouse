use bro_xml::{parse, parse_with_options, BroDocument, BroError, ParseOptions};

#[test]
fn preserves_common_metadata_and_optional_source() {
    let xml = include_str!("fixtures/cpt-minimal.xml");
    let document = parse_with_options(
        xml,
        ParseOptions {
            retain_source: true,
        },
    )
    .unwrap();
    let BroDocument::Cpt(cpt) = document else {
        panic!("expected CPT")
    };
    assert_eq!(cpt.common.bro_id, "CPT000000000001");
    assert_eq!(cpt.common.position.as_ref().unwrap().crs, "EPSG:28992");
    assert_eq!(cpt.source_xml.as_deref(), Some(xml));
}

#[test]
fn malformed_xml_has_a_position() {
    let error = parse("<CPT_O><broken></CPT_O>").unwrap_err();
    assert!(matches!(
        error,
        BroError::InvalidXml {
            position: Some(_),
            ..
        }
    ));
}
