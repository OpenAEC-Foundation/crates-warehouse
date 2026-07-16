use bro_xml::{detect, BroDocumentType, SchemaVersion};

#[test]
fn detects_all_supported_documents() {
    let cases = [
        (
            include_str!("fixtures/cpt-minimal.xml"),
            BroDocumentType::Cpt,
            SchemaVersion::new(1, 1),
        ),
        (
            include_str!("fixtures/bhr-gt-minimal.xml"),
            BroDocumentType::BhrGt,
            SchemaVersion::new(2, 1),
        ),
        (
            include_str!("fixtures/bhr-g-minimal.xml"),
            BroDocumentType::BhrG,
            SchemaVersion::new(3, 1),
        ),
    ];
    for (xml, expected_type, expected_version) in cases {
        let detected = detect(xml).expect("fixture must be detected");
        assert_eq!(detected.document_type, expected_type);
        assert_eq!(detected.schema_version, expected_version);
    }
}

#[test]
fn rejects_non_bro_xml() {
    let error = detect("<project><name>x</name></project>").unwrap_err();
    assert!(matches!(
        error,
        bro_xml::BroError::UnsupportedDocument { .. }
    ));
}

#[test]
fn rejects_supported_local_name_in_wrong_namespace_family() {
    let error = detect(r#"<CPT_O xmlns="https://example.invalid/1.1" />"#).unwrap_err();
    assert!(matches!(
        error,
        bro_xml::BroError::UnsupportedSchema {
            document: BroDocumentType::Cpt,
            ..
        }
    ));
}

#[test]
fn rejects_unsupported_schema_version() {
    let error =
        detect(r#"<BHR_GT_O xmlns="http://www.broservices.nl/xsd/dsbhr-gt/9.9" />"#).unwrap_err();
    assert!(matches!(
        error,
        bro_xml::BroError::UnsupportedSchema {
            document: BroDocumentType::BhrGt,
            ..
        }
    ));
}

#[test]
fn rejects_supported_local_name_without_namespace() {
    let error = detect("<BHR_G_O />").unwrap_err();
    assert!(matches!(
        error,
        bro_xml::BroError::UnsupportedSchema {
            document: BroDocumentType::BhrG,
            ..
        }
    ));
}

#[test]
fn rejects_malformed_xml() {
    let error =
        detect(r#"<CPT_O xmlns="http://www.broservices.nl/xsd/dscpt/1.1"><broken></CPT_O>"#)
            .unwrap_err();
    assert!(matches!(error, bro_xml::BroError::InvalidXml { .. }));
}
