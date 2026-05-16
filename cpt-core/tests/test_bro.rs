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

#[test]
fn points_are_sorted_by_depth() {
    // The cpt_bro fixture is naturally already sorted; this test guards
    // against a regression of the post-parse sort step in `bro::parse`.
    let xml = read_fixture("cpt_bro.xml");
    let cpt = parse(&xml).expect("cpt_bro.xml should parse");
    for w in cpt.points.windows(2) {
        assert!(
            w[0].depth <= w[1].depth,
            "points not sorted: {} > {}",
            w[0].depth,
            w[1].depth
        );
    }
}

#[test]
fn unsorted_bro_data_block_is_sorted_after_parse() {
    // Real-world BRO file (CPT000000000787) has ~6 depth inversions inside
    // the <values> array. Without the post-parse sort, the chart renders
    // criss-crossing curves. Confirm the parser produces monotonic depths.
    let xml = read_fixture("cpt_bro_unsorted.xml");
    let cpt = parse(&xml).expect("cpt_bro_unsorted.xml should parse");
    assert!(cpt.points.len() > 600, "expected ~657 points");
    for w in cpt.points.windows(2) {
        assert!(
            w[0].depth <= w[1].depth,
            "unsorted depths leaked through: {} > {}",
            w[0].depth,
            w[1].depth
        );
    }
}
