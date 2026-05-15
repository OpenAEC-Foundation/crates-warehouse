//! Integration tests: parse real-world JSON examples against schema types.

use std::path::Path;

use openaec_core::schema::{ContentBlock, ReportData};

/// Helper to find fixtures dir (relative to workspace root).
fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .leak()
}

#[test]
fn test_parse_example_structural() {
    let path = fixtures_dir().join("example_structural.json");
    let report =
        ReportData::from_json_file(&path).expect("Failed to parse example_structural.json");

    // Top-level fields
    assert_eq!(report.template, "structural");
    assert_eq!(report.project, "Kijkduin Reddingspost");
    assert_eq!(report.project_number.as_deref(), Some("2024-031"));
    assert_eq!(report.client.as_deref(), Some("Gemeente Den Haag"));
    assert_eq!(report.author, "3BM Bouwkunde");

    // Cover
    let cover = report.cover.as_ref().expect("Cover should exist");
    assert_eq!(
        cover.subtitle.as_deref(),
        Some("Constructieve berekening hoofddraagconstructie")
    );
    assert!(cover.extra_fields.contains_key("Kenmerk"));

    // Colofon
    let colofon = report.colofon.as_ref().expect("Colofon should exist");
    assert!(colofon.enabled);
    assert_eq!(colofon.revision_history.len(), 2);
    assert_eq!(colofon.revision_history[0].version, "0.1");

    // TOC
    let toc = report.toc.as_ref().expect("TOC should exist");
    assert!(toc.enabled);
    assert_eq!(toc.max_depth, 3);

    // Sections
    assert!(!report.sections.is_empty(), "Should have sections");
    let first_section = &report.sections[0];
    assert_eq!(first_section.title, "Uitgangspunten");
    assert_eq!(first_section.level, 1);
    assert!(!first_section.content.is_empty());

    // Count block types across all sections
    let mut paragraph_count = 0;
    let mut calc_count = 0;
    let mut check_count = 0;
    let mut table_count = 0;
    let mut spacer_count = 0;
    let mut bullet_count = 0;
    let mut heading2_count = 0;

    for section in &report.sections {
        for block in &section.content {
            match block {
                ContentBlock::Paragraph(_) => paragraph_count += 1,
                ContentBlock::Calculation(_) => calc_count += 1,
                ContentBlock::Check(_) => check_count += 1,
                ContentBlock::Table(_) => table_count += 1,
                ContentBlock::Spacer(_) => spacer_count += 1,
                ContentBlock::BulletList(_) => bullet_count += 1,
                ContentBlock::Heading2(_) => heading2_count += 1,
                _ => {}
            }
        }
    }

    // The example should contain a mix of block types
    assert!(paragraph_count > 0, "Should have paragraphs");
    assert!(calc_count > 0, "Should have calculations");
    assert!(check_count > 0, "Should have checks");
    assert!(table_count > 0, "Should have tables");

    println!("Parsed example_structural.json successfully:");
    println!("  Sections: {}", report.sections.len());
    println!("  Paragraphs: {paragraph_count}");
    println!("  Calculations: {calc_count}");
    println!("  Checks: {check_count}");
    println!("  Tables: {table_count}");
    println!("  Spacers: {spacer_count}");
    println!("  Bullet lists: {bullet_count}");
    println!("  Heading2s: {heading2_count}");
}

#[test]
fn test_roundtrip_example() {
    let path = fixtures_dir().join("example_structural.json");
    let report = ReportData::from_json_file(&path).expect("Failed to parse");

    // Serialize back to JSON and re-parse
    let json = report.to_json().expect("Failed to serialize");
    let reparsed = ReportData::from_json(&json).expect("Failed to re-parse");

    assert_eq!(reparsed.template, report.template);
    assert_eq!(reparsed.project, report.project);
    assert_eq!(reparsed.sections.len(), report.sections.len());
}
