mod common;

use cpt_core::{parse_auto, render_cpt_svg};
use common::read_fixture;

#[test]
fn renders_voorbeeld_to_svg() {
    let cpt = parse_auto(&read_fixture("voorbeeld.gef")).unwrap();
    let svg = render_cpt_svg(&cpt);

    // Sanity: SVG root element present
    assert!(svg.starts_with("<svg") || svg.starts_with("<?xml"));
    assert!(svg.contains("</svg>"));

    // Sanity: must include the qc curve in some form
    assert!(svg.contains("polyline") || svg.contains("path"));

    // Sanity: at least one Robertson colour appears
    assert!(
        ["#FF9800", "#4CAF50", "#FFC107", "#FF5722", "#8BC34A", "#795548", "#00BCD4", "#F44336", "#9C27B0"]
            .iter().any(|c| svg.contains(c)),
        "expected at least one Robertson colour in the SVG"
    );
}

#[test]
fn handles_empty_cpt() {
    let cpt = cpt_core::Cpt {
        id: "empty".into(),
        metadata: cpt_core::Metadata { source_file: "x".into(), ..Default::default() },
        position: None,
        points: vec![],
    };
    let svg = render_cpt_svg(&cpt);
    // Should still produce a valid empty-state SVG, not panic
    assert!(svg.contains("<svg"));
}
