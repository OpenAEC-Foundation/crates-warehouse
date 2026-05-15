mod common;

use cpt_core::parse_auto;
use common::read_fixture;

#[test]
fn dispatches_gef_by_prefix() {
    let text = read_fixture("voorbeeld.gef");
    let cpt = parse_auto(&text).unwrap();
    assert!(!cpt.points.is_empty());
}

#[test]
fn dispatches_xml_by_prefix() {
    let text = read_fixture("cpt_bro.xml");
    let cpt = parse_auto(&text).unwrap();
    assert!(!cpt.points.is_empty());
}

#[test]
fn rejects_unknown_format() {
    let result = parse_auto("hello world\nthis is not a CPT file");
    assert!(matches!(result, Err(cpt_core::CptError::UnknownFormat)));
}
