use bro_xml::{
    parse_bhr_g, parse_bhr_g_with_options, parse_bhr_gt, parse_bhr_gt_with_options,
    BroDocumentType, BroError, ParseOptions,
};
use chrono::NaiveDate;

#[test]
fn parses_geotechnical_intervals() {
    let bore = parse_bhr_gt(include_str!("fixtures/bhr-gt-minimal.xml")).unwrap();
    assert_eq!(bore.common.bro_id, "BHR000000000001");
    assert_eq!(bore.intervals.len(), 2);
    assert_eq!(bore.final_depth, Some(4.0));
    assert_eq!(
        bore.intervals[0].soil_name.as_deref(),
        Some("sterkSiltigeKlei")
    );
    assert!(bore.intervals[0].upper_boundary < bore.intervals[0].lower_boundary);
}

#[test]
fn parses_geological_intervals_without_wrapper_duplicates() {
    let bore = parse_bhr_g(include_str!("fixtures/bhr-g-minimal.xml")).unwrap();
    assert_eq!(bore.common.bro_id, "BHR000000000002");
    assert_eq!(bore.intervals.len(), 2);
    assert_eq!(bore.final_depth, Some(6.0));
    assert_eq!(bore.intervals[0].lithology.as_deref(), Some("zand"));
}

#[test]
fn deduplication_preserves_fields_from_geological_wrappers() {
    let xml = include_str!("fixtures/bhr-g-minimal.xml").replacen(
        "    <layer>\n      <layer>",
        "    <layer>\n      <description>veldwaarneming</description>\n      <layer>",
        1,
    );
    let bore = parse_bhr_g(&xml).unwrap();

    assert_eq!(bore.intervals.len(), 2);
    assert_eq!(
        bore.intervals[0].description.as_deref(),
        Some("veldwaarneming")
    );
}

#[test]
fn collects_all_geotechnical_secondary_attribute_categories() {
    let xml = include_str!("fixtures/bhr-gt-minimal.xml").replace(
        "      <organicMatterContentClass>zwakHumeus</organicMatterContentClass>",
        r#"      <anomalousLayer>schelp</anomalousLayer>
      <chunks>grind</chunks>
      <peatFraction>weinig</peatFraction>
      <pedologicalSoilName>eerdgrond</pedologicalSoilName>
      <organicMatterContentClass>zwakHumeus</organicMatterContentClass>
      <carbonateContentClass>kalkrijk</carbonateContentClass>
      <ripening>gerijpt</ripening>
      <soilStructure>massief</soilStructure>
      <horizonValue>Ah</horizonValue>"#,
    );
    let bore = parse_bhr_gt(&xml).unwrap();
    let secondary = &bore.intervals[0].secondary;

    for (code, value) in [
        ("anomalousLayer", "schelp"),
        ("chunks", "grind"),
        ("peatFraction", "weinig"),
        ("pedologicalSoilName", "eerdgrond"),
        ("organicMatterContentClass", "zwakHumeus"),
        ("carbonateContentClass", "kalkrijk"),
        ("ripening", "gerijpt"),
        ("soilStructure", "massief"),
        ("horizonValue", "Ah"),
    ] {
        assert!(
            secondary
                .iter()
                .any(|attribute| attribute.code == code && attribute.value == value),
            "missing {code}"
        );
    }
}

#[test]
fn keeps_pedological_soil_name_secondary_without_using_it_as_primary_soil_name() {
    let xml = include_str!("fixtures/bhr-gt-minimal.xml").replace(
        "<geotechnicalSoilName>sterkSiltigeKlei</geotechnicalSoilName>",
        "<pedologicalSoilName>eerdgrond</pedologicalSoilName>",
    );
    let bore = parse_bhr_gt(&xml).unwrap();

    assert_eq!(bore.intervals[0].soil_name, None);
    assert!(
        bore.intervals[0]
            .secondary
            .iter()
            .any(|attribute| attribute.code == "pedologicalSoilName"
                && attribute.value == "eerdgrond")
    );
}

#[test]
fn preserves_primitive_geological_interval_extensions() {
    let bore = parse_bhr_g(include_str!("fixtures/bhr-g-minimal.xml")).unwrap();

    assert_eq!(
        bore.intervals[0]
            .extensions
            .get("grainSize")
            .map(String::as_str),
        Some("matigFijn")
    );
    assert_eq!(
        bore.intervals[1]
            .extensions
            .get("shellContent")
            .map(String::as_str),
        Some("weinig")
    );
}

#[test]
fn typed_borehole_parsers_reject_the_other_document_types() {
    assert!(matches!(
        parse_bhr_gt(include_str!("fixtures/bhr-g-minimal.xml")),
        Err(BroError::UnexpectedDocumentType {
            expected: BroDocumentType::BhrGt,
            found: BroDocumentType::BhrG,
        })
    ));
    assert!(matches!(
        parse_bhr_g(include_str!("fixtures/bhr-gt-minimal.xml")),
        Err(BroError::UnexpectedDocumentType {
            expected: BroDocumentType::BhrG,
            found: BroDocumentType::BhrGt,
        })
    ));
}

#[test]
fn source_xml_is_retained_only_when_requested() {
    let gt_xml = include_str!("fixtures/bhr-gt-minimal.xml");
    let g_xml = include_str!("fixtures/bhr-g-minimal.xml");

    assert_eq!(parse_bhr_gt(gt_xml).unwrap().source_xml, None);
    assert_eq!(parse_bhr_g(g_xml).unwrap().source_xml, None);
    assert_eq!(
        parse_bhr_gt_with_options(
            gt_xml,
            ParseOptions {
                retain_source: true
            }
        )
        .unwrap()
        .source_xml
        .as_deref(),
        Some(gt_xml)
    );
    assert_eq!(
        parse_bhr_g_with_options(
            g_xml,
            ParseOptions {
                retain_source: true
            }
        )
        .unwrap()
        .source_xml
        .as_deref(),
        Some(g_xml)
    );
}

#[test]
fn sorts_geotechnical_intervals_by_upper_boundary() {
    let xml = include_str!("fixtures/bhr-gt-minimal.xml")
        .replace(
            "<upperBoundary>0.0</upperBoundary>",
            "<upperBoundary>4.0</upperBoundary>",
        )
        .replace(
            "<lowerBoundary>1.5</lowerBoundary>",
            "<lowerBoundary>5.0</lowerBoundary>",
        )
        .replace(
            "<upperBoundary>1.5</upperBoundary>",
            "<upperBoundary>0.0</upperBoundary>",
        );
    let bore = parse_bhr_gt(&xml).unwrap();

    assert_eq!(bore.intervals[0].upper_boundary, 0.0);
    assert_eq!(bore.intervals[1].upper_boundary, 4.0);
}

#[test]
fn sorts_geological_intervals_by_upper_boundary() {
    let xml = include_str!("fixtures/bhr-g-minimal.xml")
        .replace(
            "<upperBoundary>0.0</upperBoundary>",
            "<upperBoundary>6.0</upperBoundary>",
        )
        .replace(
            "<lowerBoundary>2.0</lowerBoundary>",
            "<lowerBoundary>7.0</lowerBoundary>",
        )
        .replace(
            "<upperBoundary>2.0</upperBoundary>",
            "<upperBoundary>0.0</upperBoundary>",
        );
    let bore = parse_bhr_g(&xml).unwrap();

    assert_eq!(bore.intervals[0].upper_boundary, 0.0);
    assert_eq!(bore.intervals[1].upper_boundary, 6.0);
}

#[test]
fn rejects_non_finite_geotechnical_interval_boundary_with_slash_path() {
    let non_finite = include_str!("fixtures/bhr-gt-minimal.xml").replace(
        "<upperBoundary>0.0</upperBoundary>",
        "<upperBoundary>NaN</upperBoundary>",
    );
    assert!(matches!(
        parse_bhr_gt(&non_finite),
        Err(BroError::InvalidValue { path, .. }) if path.contains("/layer/upperBoundary")
    ));
}

#[test]
fn rejects_reversed_geotechnical_interval_boundary_with_slash_path() {
    let reversed = include_str!("fixtures/bhr-gt-minimal.xml").replacen(
        "<lowerBoundary>1.5</lowerBoundary>",
        "<lowerBoundary>0.0</lowerBoundary>",
        1,
    );
    assert!(matches!(
        parse_bhr_gt(&reversed),
        Err(BroError::InvalidValue { path, .. }) if path.contains("/layer/lowerBoundary")
    ));
}

#[test]
fn rejects_non_finite_geological_interval_boundary_with_slash_path() {
    let non_finite = include_str!("fixtures/bhr-g-minimal.xml").replace(
        "<upperBoundary>0.0</upperBoundary>",
        "<upperBoundary>NaN</upperBoundary>",
    );
    assert!(matches!(
        parse_bhr_g(&non_finite),
        Err(BroError::InvalidValue { path, .. }) if path.contains("/layer/layer/upperBoundary")
    ));
}

#[test]
fn rejects_reversed_geological_interval_boundary_with_slash_path() {
    let reversed = include_str!("fixtures/bhr-g-minimal.xml").replacen(
        "<lowerBoundary>2.0</lowerBoundary>",
        "<lowerBoundary>0.0</lowerBoundary>",
        1,
    );
    assert!(matches!(
        parse_bhr_g(&reversed),
        Err(BroError::InvalidValue { path, .. }) if path.contains("/layer/layer/lowerBoundary")
    ));
}

#[test]
fn maps_current_bhr_g_soil_name_to_lithology() {
    let bore = parse_bhr_g(include_str!("fixtures/bhr-g-dispatch.xml")).unwrap();

    assert_eq!(bore.intervals.len(), 1);
    assert_eq!(
        bore.intervals[0].lithology.as_deref(),
        Some("matigFijnZand")
    );
}

#[test]
fn reads_nested_boring_dates_without_using_unrelated_date_leaf() {
    let bore = parse_bhr_g(include_str!("fixtures/bhr-g-dispatch.xml")).unwrap();

    assert_eq!(
        bore.common.research_start_date,
        Some(NaiveDate::from_ymd_opt(2026, 5, 4).unwrap())
    );
    assert_eq!(
        bore.common.research_end_date,
        Some(NaiveDate::from_ymd_opt(2026, 5, 6).unwrap())
    );
}
