mod common;

use cpt_core::bro::parse;
use common::read_fixture;

#[test]
fn parses_cpt_bro_xml() {
    let xml = read_fixture("cpt_bro.xml");
    let cpt = parse(&xml).expect("cpt_bro.xml should parse");
    assert!(!cpt.id.is_empty());
    assert!(!cpt.points.is_empty(), "expected at least one measurement point");
    let first = &cpt.points[0];
    assert!(first.depth >= 0.0);
    assert!(first.qc.is_some());
}

#[test]
fn applies_bro_void_value() {
    let xml = read_fixture("cpt_bro.xml");
    let cpt = parse(&xml).unwrap();
    // No -999999 should leak through into any field
    for p in &cpt.points {
        for v in [p.qc, p.fs, p.rf, p.u2, p.inclination].iter().flatten() {
            assert!(*v > -100_000.0, "void value leaked through: {}", v);
        }
    }
}
