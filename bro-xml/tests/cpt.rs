use bro_xml::{parse_cpt, BroDocumentType, BroError};
use chrono::NaiveDate;

const CPT_XML: &str = include_str!("fixtures/cpt-minimal.xml");

#[test]
fn parses_exact_cpt_columns_and_sorts_by_depth_not_length() {
    let cpt = parse_cpt(CPT_XML).unwrap();

    assert_eq!(cpt.common.bro_id, "CPT000000000001");
    assert_eq!(cpt.measurements.len(), 2);
    let point = cpt.measurements[0];
    assert_eq!(point.depth, 1.0);
    assert_eq!(point.cone_resistance, Some(13.3));
    assert_eq!(point.inclination, Some(115.15));
    assert_eq!(point.sleeve_friction, Some(118.18));
    assert_eq!(point.pore_pressure_u2, Some(122.22));
    assert_eq!(point.friction_ratio, Some(124.24));
    assert_eq!(cpt.measurements[1].depth, 2.0);
}

#[test]
fn converts_each_optional_void_representation_to_none() {
    let cpt = parse_cpt(CPT_XML).unwrap();
    let point = cpt.measurements[1];

    assert_eq!(point.cone_resistance, None, "empty cell");
    assert_eq!(point.inclination, None, "BRO sentinel");
    assert_eq!(point.sleeve_friction, None, "NaN");
    assert_eq!(point.pore_pressure_u2, None, "positive infinity");
    assert_eq!(point.friction_ratio, None, "negative infinity");
}

#[test]
fn rejects_non_cpt_documents_with_a_typed_error() {
    for (xml, found) in [
        (
            include_str!("fixtures/bhr-gt-minimal.xml"),
            BroDocumentType::BhrGt,
        ),
        (
            include_str!("fixtures/bhr-g-minimal.xml"),
            BroDocumentType::BhrG,
        ),
    ] {
        assert!(matches!(
            parse_cpt(xml),
            Err(BroError::UnexpectedDocumentType {
                expected: BroDocumentType::Cpt,
                found: actual,
            }) if actual == found
        ));
    }
}

#[test]
fn rejects_missing_result_block() {
    let xml = replace_result(CPT_XML, "");
    assert!(matches!(
        parse_cpt(&xml),
        Err(BroError::MissingField { .. })
    ));
}

#[test]
fn rejects_empty_result_block() {
    let xml = replace_result(CPT_XML, "<result />");
    assert!(matches!(
        parse_cpt(&xml),
        Err(BroError::MissingField { .. })
    ));
}

#[test]
fn rejects_values_outside_a_cpt_result_data_array() {
    let xml = replace_result(
        CPT_XML,
        "<extension><values>0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24</values></extension>",
    );
    assert!(matches!(
        parse_cpt(&xml),
        Err(BroError::MissingField { .. })
    ));
}

#[test]
fn rejects_rows_with_24_or_26_columns() {
    for count in [24, 26] {
        let row = (0..count)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let xml = replace_values(CPT_XML, &row);
        assert!(matches!(
            parse_cpt(&xml),
            Err(BroError::InvalidValue { .. })
        ));
    }
}

#[test]
fn rejects_void_and_non_finite_depths() {
    for depth in ["", "-999999", "NaN", "inf", "-inf"] {
        let row =
            format!("999,{depth},2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24");
        let xml = replace_values(CPT_XML, &row);
        assert!(matches!(
            parse_cpt(&xml),
            Err(BroError::InvalidValue { .. })
        ));
    }
}

#[test]
fn converts_bro_void_final_depth_to_none() {
    let xml = CPT_XML.replace(
        "<finalDepth>2.0</finalDepth>",
        "<finalDepth>-999999</finalDepth>",
    );
    assert_eq!(parse_cpt(&xml).unwrap().final_depth, None);
}

#[test]
fn uses_delivered_position_when_standardized_position_comes_first() {
    let cpt = parse_cpt(include_str!("fixtures/cpt-dispatch-location.xml")).unwrap();
    let position = cpt.common.position.unwrap();

    assert_eq!(position.x, 155_123.4);
    assert_eq!(position.y, 463_567.8);
    assert_eq!(position.crs, "urn:ogc:def:crs:EPSG::28992");
}

#[test]
fn reads_current_cone_penetrometer_type_field() {
    let cpt = parse_cpt(include_str!("fixtures/cpt-dispatch-location.xml")).unwrap();

    assert_eq!(cpt.cone_type.as_deref(), Some("electrical"));
}

#[test]
fn reads_date_leaf_nested_below_research_start_date() {
    let cpt = parse_cpt(include_str!("fixtures/cpt-dispatch-location.xml")).unwrap();

    assert_eq!(
        cpt.common.research_start_date,
        Some(NaiveDate::from_ymd_opt(2026, 4, 3).unwrap())
    );
}

fn replace_values(xml: &str, replacement: &str) -> String {
    let start = xml.find("<swe:values>").unwrap() + "<swe:values>".len();
    let end = xml.find("</swe:values>").unwrap();
    format!("{}{replacement}{}", &xml[..start], &xml[end..])
}

fn replace_result(xml: &str, replacement: &str) -> String {
    let start = xml.find("    <result>").unwrap();
    let end = xml.find("    </result>").unwrap() + "    </result>".len();
    format!("{}{replacement}{}", &xml[..start], &xml[end..])
}
