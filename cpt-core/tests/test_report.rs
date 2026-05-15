mod common;

use cpt_core::{parse_auto, build_report, ProjectMeta};
use common::read_fixture;

#[test]
fn builds_report_for_one_cpt() {
    let cpt = parse_auto(&read_fixture("voorbeeld.gef")).unwrap();
    let project = ProjectMeta {
        title: "Voorbeeld project".into(),
        client: "ACME bv".into(),
        location: "Amsterdam".into(),
        project_number: "2026-001".into(),
        author: "Open GEO Studio".into(),
        date: chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
    };
    let report = build_report(&[cpt], &project);
    assert_eq!(report.project, "Voorbeeld project");
    assert!(report.cover.is_some());
    // Sections: at least coordinate table + 1 per CPT page
    assert!(report.sections.len() >= 2, "got {} sections", report.sections.len());
}

#[test]
fn report_serializes_to_json() {
    let cpt = parse_auto(&read_fixture("cpt_bro.xml")).unwrap();
    let project = ProjectMeta {
        title: "T".into(), client: "C".into(), location: "L".into(),
        project_number: "P".into(), author: "A".into(),
        date: chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
    };
    let report = build_report(&[cpt], &project);
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"project\":\"T\""));
}
