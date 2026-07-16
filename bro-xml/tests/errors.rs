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

#[test]
fn bhr_gt_dispatch_stops_at_first_document_specific_field() {
    let xml = r#"<BHR_GT_O xmlns="http://www.broservices.nl/xsd/dsbhr-gt/2.1">
        <broId>BHRGT000000000001</broId>
    </BHR_GT_O>"#;

    let error = parse(xml).unwrap_err();

    assert!(matches!(
        error,
        BroError::MissingField { ref path } if path == "BHR_GT_O/boring"
    ));
}

#[test]
fn bhr_g_dispatch_stops_at_first_document_specific_field() {
    let xml = r#"<BHR_G_O xmlns="http://www.broservices.nl/xsd/dsbhrg/3.1">
        <broId>BHRG000000000001</broId>
    </BHR_G_O>"#;

    let error = parse(xml).unwrap_err();

    assert!(matches!(
        error,
        BroError::MissingField { ref path } if path == "BHR_G_O/boring"
    ));
}

#[test]
fn scalar_diagnostics_use_collected_paths() {
    let missing_bro_id =
        parse(r#"<CPT_O xmlns="http://www.broservices.nl/xsd/dscpt/1.1" />"#).unwrap_err();
    assert!(matches!(
        missing_bro_id,
        BroError::MissingField { ref path } if path == "CPT_O/broId"
    ));

    let invalid_date = parse(
        r#"<CPT_O xmlns="http://www.broservices.nl/xsd/dscpt/1.1">
            <broId>CPT000000000001</broId>
            <researchStartDate>not-a-date</researchStartDate>
        </CPT_O>"#,
    )
    .unwrap_err();
    assert!(matches!(
        invalid_date,
        BroError::InvalidValue { ref path, .. }
            if path == "CPT_O/researchStartDate"
    ));

    let invalid_offset = parse(
        r#"<CPT_O xmlns="http://www.broservices.nl/xsd/dscpt/1.1">
            <broId>CPT000000000001</broId>
            <deliveredVerticalPosition><offset>not-a-number</offset></deliveredVerticalPosition>
        </CPT_O>"#,
    )
    .unwrap_err();
    assert!(matches!(
        invalid_offset,
        BroError::InvalidValue { ref path, .. }
            if path == "CPT_O/deliveredVerticalPosition/offset"
    ));
}

#[test]
fn extension_limit_counts_characters_instead_of_bytes() {
    let value = "é".repeat(199);
    let xml = include_str!("fixtures/cpt-minimal.xml").replace(
        "</CPT_O>",
        &format!("<customValue>{value}</customValue></CPT_O>"),
    );

    let document = parse(&xml).unwrap();
    let BroDocument::Cpt(cpt) = document else {
        panic!("expected CPT")
    };

    assert_eq!(cpt.common.extensions.get("CPT_O/customValue"), Some(&value));
}
