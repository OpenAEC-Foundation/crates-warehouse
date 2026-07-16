use bro_xml::parse_cpt;

#[test]
fn parses_and_sorts_cpt_measurements() {
    let cpt = parse_cpt(include_str!("fixtures/cpt-minimal.xml")).unwrap();
    assert_eq!(cpt.common.bro_id, "CPT000000000001");
    assert_eq!(cpt.measurements.len(), 2);
    assert!(cpt.measurements[0].depth <= cpt.measurements[1].depth);
    assert_eq!(cpt.measurements[0].cone_resistance, Some(4.2));
}

#[test]
fn converts_bro_void_values_to_none() {
    let cpt = parse_cpt(include_str!("fixtures/cpt-minimal.xml")).unwrap();
    assert!(cpt
        .measurements
        .iter()
        .any(|point| point.pore_pressure_u2.is_none()));
    assert!(cpt
        .measurements
        .iter()
        .flat_map(|point| {
            [
                point.cone_resistance,
                point.sleeve_friction,
                point.friction_ratio,
                point.pore_pressure_u2,
            ]
        })
        .flatten()
        .all(|value| value > -100_000.0));
}

#[test]
fn converts_bro_void_final_depth_to_none() {
    let xml = include_str!("fixtures/cpt-minimal.xml").replace(
        "<finalDepth>2.0</finalDepth>",
        "<finalDepth>-999999</finalDepth>",
    );

    let cpt = parse_cpt(&xml).unwrap();

    assert_eq!(cpt.final_depth, None);
}
