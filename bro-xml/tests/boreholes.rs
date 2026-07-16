use bro_xml::{
    parse_bhr_g, parse_bhr_g_with_options, parse_bhr_gt, parse_bhr_gt_with_options,
    BroDocumentType, BroError, ParseOptions,
};

#[test]
fn parses_geotechnical_intervals() {
    let bore = parse_bhr_gt(include_str!("fixtures/bhr-gt-minimal.xml")).unwrap();
    assert_eq!(bore.common.bro_id, "BHR000000000001");
    assert_eq!(bore.intervals.len(), 2);
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
        "      <organicMatterContent>zwakHumeus</organicMatterContent>",
        r#"      <anomalousLayer>schelp</anomalousLayer>
      <chunks>grind</chunks>
      <peatFraction>weinig</peatFraction>
      <pedologicalSoilName>eerdgrond</pedologicalSoilName>
      <organicMatterContent>zwakHumeus</organicMatterContent>
      <carbonateContent>kalkrijk</carbonateContent>
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
        ("organicMatterContent", "zwakHumeus"),
        ("carbonateContent", "kalkrijk"),
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
fn rejects_non_finite_and_reversed_interval_boundaries_with_slash_paths() {
    let non_finite = include_str!("fixtures/bhr-gt-minimal.xml").replace(
        "<upperBoundary>0.0</upperBoundary>",
        "<upperBoundary>NaN</upperBoundary>",
    );
    assert!(matches!(
        parse_bhr_gt(&non_finite),
        Err(BroError::InvalidValue { path, .. }) if path.contains("/layer/upperBoundary")
    ));

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
