use cpt_core::coords::{rd_to_wgs84, wgs84_to_rd};

#[test]
fn rd_to_wgs84_amersfoort_origin() {
    // Amersfoort RD origin: (155000, 463000) -> (52.155, 5.387) approx
    let (lat, lon) = rd_to_wgs84(155_000.0, 463_000.0);
    assert!((lat - 52.1551744).abs() < 1e-4, "lat {} off", lat);
    assert!((lon - 5.3872036).abs() < 1e-4, "lon {} off", lon);
}

#[test]
fn rd_to_wgs84_dordrecht() {
    // ~Dordrecht: (106800, 425250) -> ~(51.815, 4.690)
    let (lat, lon) = rd_to_wgs84(106_800.0, 425_250.0);
    assert!((lat - 51.815).abs() < 0.01, "lat {} off", lat);
    assert!((lon - 4.690).abs() < 0.01, "lon {} off", lon);
}

#[test]
fn wgs84_to_rd_round_trip() {
    let original = (155_000.0_f64, 463_000.0_f64);
    let (lat, lon) = rd_to_wgs84(original.0, original.1);
    let (x, y) = wgs84_to_rd(lat, lon);
    assert!((x - original.0).abs() < 0.5, "x roundtrip {} off", x);
    assert!((y - original.1).abs() < 0.5, "y roundtrip {} off", y);
}
