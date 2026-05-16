use cpt_core::{Cpt, MeasurementPoint, Metadata, Position};

#[test]
fn cpt_serializes_to_json() {
    let cpt = Cpt {
        id: "S01".to_string(),
        metadata: Metadata {
            project_name: Some("Test Project".to_string()),
            project_number: Some("2026-001".to_string()),
            date: chrono::NaiveDate::from_ymd_opt(2026, 5, 15),
            equipment: None,
            ground_level_nap: Some(2.5),
            source_file: "test.gef".to_string(),
            ..Default::default()
        },
        position: Some(Position {
            x_rd: 100_000.0,
            y_rd: 400_000.0,
            z_nap: Some(2.5),
        }),
        points: vec![MeasurementPoint {
            depth: 0.5,
            depth_nap: Some(2.0),
            qc: Some(1.2),
            fs: Some(0.012),
            rf: Some(1.0),
            u2: None,
            inclination: Some(0.5),
        }],
    };
    let json = serde_json::to_string(&cpt).unwrap();
    let back: Cpt = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "S01");
    assert_eq!(back.points.len(), 1);
    assert_eq!(back.points[0].qc, Some(1.2));
}
