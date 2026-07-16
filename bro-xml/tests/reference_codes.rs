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

#[test]
fn describes_bhr_g_specific_colour_code() {
    assert_eq!(
        describe_reference_code(ReferenceCodeSet::Colour, "roze"),
        Some(
            "Roze omvat de Munsellkleuren 10R 8/3, 10R 8/4, 2.5YR 8/3, 2.5YR 8/4, 5YR 7/3, 5YR 7/4, 5YR 8/3, 5YR 8/4, 7.5YR 7/3, 7.5YR 7/4, 7.5YR 8/3 en 7.5YR 8/4 (pink)."
        )
    );
}
