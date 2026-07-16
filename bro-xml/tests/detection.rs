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
