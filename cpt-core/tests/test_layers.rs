use cpt_core::{detect_layers, Cpt, MeasurementPoint, Metadata};

fn make_cpt(points: Vec<(f64, f64, f64)>) -> Cpt {
    Cpt {
        id: "T".to_string(),
        metadata: Metadata { source_file: "test.gef".to_string(), ..Default::default() },
        position: None,
        points: points.into_iter().map(|(d, qc, rf)| MeasurementPoint {
            depth: d,
            depth_nap: None,
            qc: Some(qc),
            fs: None,
            rf: Some(rf),
            u2: None,
            inclination: None,
        }).collect(),
    }
}

#[test]
fn empty_cpt_has_no_layers() {
    let cpt = make_cpt(vec![]);
    assert_eq!(detect_layers(&cpt).len(), 0);
}

#[test]
fn uniform_zone_collapses_to_one_layer() {
    // All points classify to the same zone (zone 6, Zand)
    let pts = (0..50).map(|i| (i as f64 * 0.02, 8.0, 0.7)).collect();
    let cpt = make_cpt(pts);
    let layers = detect_layers(&cpt);
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].zone_number, 6); // Zand
    assert!((layers[0].depth_top - 0.0).abs() < 1e-9);
    assert!((layers[0].depth_bottom - 0.98).abs() < 1e-9);
}

#[test]
fn distinct_zones_become_separate_layers() {
    // First half: Zand (zone 6), second half: zone 2 or 3
    let mut pts = vec![];
    for i in 0..20 { pts.push((i as f64 * 0.1, 8.0, 0.7)); }    // 0..1.9 m, zone 6
    for i in 20..40 { pts.push((i as f64 * 0.1, 1.5, 6.0)); }   // 2..3.9 m, zone 2 or 3
    let cpt = make_cpt(pts);
    let layers = detect_layers(&cpt);
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].zone_number, 6);
}

#[test]
fn thin_layer_below_threshold_is_merged() {
    // 50cm zand, 6cm "klei" (below 10cm), 50cm zand → one merged zand layer
    let mut pts = vec![];
    for i in 0..25 { pts.push((i as f64 * 0.02, 8.0, 0.7)); }       // 0.5m zand
    for i in 25..28 { pts.push((i as f64 * 0.02, 1.5, 6.0)); }      // 6cm thin
    for i in 28..53 { pts.push((i as f64 * 0.02, 8.0, 0.7)); }      // 0.5m zand
    let cpt = make_cpt(pts);
    let layers = detect_layers(&cpt);
    assert_eq!(layers.len(), 1, "thin layer should merge into surrounding zand");
    assert_eq!(layers[0].zone_number, 6);
}
