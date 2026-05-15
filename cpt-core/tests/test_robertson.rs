use cpt_core::robertson::{classify, zones, Zone};

#[test]
fn classify_returns_none_for_invalid_input() {
    assert!(classify(0.0, 1.0).is_none());
    assert!(classify(-1.0, 1.0).is_none());
    assert!(classify(1.0, -0.1).is_none());
}

#[test]
fn classify_high_qc_low_rf_is_grof_zand() {
    // qc > 25, Rf < 1 -> Zone 7 (Grof zand / grind)
    let z = classify(30.0, 0.5).unwrap();
    assert_eq!(z.number, 7);
}

#[test]
fn classify_medium_qc_medium_rf_is_zand() {
    // qc = 8, Rf = 0.7 -> Zone 6 (Zand)
    let z = classify(8.0, 0.7).unwrap();
    assert_eq!(z.number, 6);
}

#[test]
fn classify_low_qc_high_rf_is_klei_or_organic() {
    // qc = 1.5, Rf = 6 -> Zone 3 (Klei) or Zone 2 (Organisch)
    let z = classify(1.5, 6.0).unwrap();
    assert!(z.number == 2 || z.number == 3, "got zone {}", z.number);
}

#[test]
fn zones_returns_nine_entries() {
    assert_eq!(zones().len(), 9);
}

#[test]
fn zones_have_unique_numbers() {
    let nums: std::collections::HashSet<u8> = zones().iter().map(|z| z.number).collect();
    assert_eq!(nums.len(), 9);
}

#[test]
fn zone_serializes_to_json() {
    let z = Zone { number: 3, name: "Klei", color: "#4CAF50" };
    let json = serde_json::to_string(&z).unwrap();
    assert!(json.contains("\"number\":3"));
    assert!(json.contains("\"name\":\"Klei\""));
}
