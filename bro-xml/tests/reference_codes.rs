use bro_xml::{describe_reference_code, ReferenceCodeSet};

#[test]
fn describes_known_soil_code_and_preserves_unknown_codes() {
    assert!(
        describe_reference_code(ReferenceCodeSet::GeotechnicalSoilName, "sterkSiltigeKlei")
            .is_some()
    );
    assert_eq!(
        describe_reference_code(ReferenceCodeSet::GeotechnicalSoilName, "futureCode"),
        None
    );
}
