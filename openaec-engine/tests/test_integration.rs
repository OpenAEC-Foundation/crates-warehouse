//! Integration test: generate a test PDF with the engine.

use std::collections::HashMap;
use openaec_core::brand::BrandConfig;
use openaec_core::template_config::*;
use openaec_engine::engine::Engine;
use openaec_engine::pdf_backend::PdfBackend;
use openaec_engine::font_engine::FontEngine;
use openaec_engine::zone_renderer;
use openaec_engine::text;
use serde_json::json;

/// Generate a simple test PDF with text, lines, and rectangles.
#[test]
fn test_generate_simple_pdf() {
    let mut backend = PdfBackend::new();

    // Add an A4 page
    let page = backend.add_page(210.0, 297.0);
    let page_height_pt = 297.0 * 2.834_645_669_3;

    // Draw a colored rectangle (header bar)
    backend.draw_rect(
        page,
        0.0, (page_height_pt - 30.0 * 2.8346) as f32,
        (210.0 * 2.8346) as f32, (30.0 * 2.8346) as f32,
        Some(openaec_engine::pdf_backend::Color::from_hex("#40124A")),
        None,
    );

    // Draw a line
    backend.draw_line(
        page,
        (20.0 * 2.8346) as f32,
        (page_height_pt - 50.0 * 2.8346) as f32,
        (190.0 * 2.8346) as f32,
        (page_height_pt - 50.0 * 2.8346) as f32,
        2.0,
        openaec_engine::pdf_backend::Color::from_hex("#38BDA0"),
    );

    // Draw another rectangle (content area)
    backend.draw_rect(
        page,
        (20.0 * 2.8346) as f32, (page_height_pt - 280.0 * 2.8346) as f32,
        (170.0 * 2.8346) as f32, (220.0 * 2.8346) as f32,
        None,
        Some((0.5, openaec_engine::pdf_backend::Color::from_hex("#CCCCCC"))),
    );

    // Finalize
    let bytes = backend.finish().unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    assert!(bytes.len() > 500);

    // Write to disk for visual inspection
    let output_path = std::path::Path::new("../../output/test_rust_engine.pdf");
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(output_path, &bytes).unwrap();
    println!("Generated PDF: {} bytes → {}", bytes.len(), output_path.display());
}

/// Test engine build with minimal template config.
#[test]
fn test_engine_build_minimal() {
    let engine = Engine::new(openaec_engine::EngineConfig::default());

    // Create minimal template
    let template = TemplateConfigV2 {
        name: "test".into(),
        tenant: "test".into(),
        pages: vec![
            PageDef {
                page_type_kind: PageDefType::Fixed,
                page_type: "page1".into(),
                orientation: "portrait".into(),
                repeat: RepeatMode::None,
            },
        ],
    };

    // Create page type with text zones
    let mut page_types = HashMap::new();
    page_types.insert("page1".into(), PageType {
        name: "page1".into(),
        stationery: None,
        text_zones: vec![
            TextZone {
                bind: "project".into(),
                x_mm: 25.0,
                y_mm: 40.0,
                font: "body".into(),
                size: 14.0,
                color: "text".into(),
                align: TextAlign::Left,
                max_width_mm: None,
                line_height_mm: 4.2,
            },
            TextZone {
                bind: "_static.Opdrachtgever:".into(),
                x_mm: 25.0,
                y_mm: 55.0,
                font: "body".into(),
                size: 10.0,
                color: "text".into(),
                align: TextAlign::Left,
                max_width_mm: None,
                line_height_mm: 4.2,
            },
            TextZone {
                bind: "client.name".into(),
                x_mm: 80.0,
                y_mm: 55.0,
                font: "body".into(),
                size: 10.0,
                color: "text".into(),
                align: TextAlign::Left,
                max_width_mm: None,
                line_height_mm: 4.2,
            },
        ],
        image_zones: Vec::new(),
        line_zones: vec![
            LineZone {
                x0_mm: 20.0,
                y_mm: 50.0,
                x1_mm: 190.0,
                width_pt: 2.0,
                color: "primary".into(),
            },
        ],
        table: None,
        content_frame: None,
        flow_layout: false,
        flow_footer_y_mm: 260.0,
        flow_content_start_y_mm: 32.0,
    });

    // Minimal brand
    let brand = BrandConfig {
        colors: {
            let mut m = HashMap::new();
            m.insert("primary".into(), "#40124A".into());
            m.insert("secondary".into(), "#38BDA0".into());
            m.insert("text".into(), "#333333".into());
            m.insert("text_light".into(), "#999999".into());
            m
        },
        fonts: HashMap::new(),
        ..Default::default()
    };

    let data = json!({
        "project": "Test Rapport — Rust Engine",
        "client": {
            "name": "OpenAEC Foundation"
        }
    });

    let stationery_dir = std::path::Path::new(".");
    let base_dir = std::path::Path::new(".");

    let result = engine.build(&template, &page_types, &brand, &data, stationery_dir, base_dir);
    assert!(result.is_ok(), "Engine build failed: {:?}", result.err());

    let bytes = result.unwrap();
    assert!(bytes.starts_with(b"%PDF"));

    // Write for inspection
    let output_path = std::path::Path::new("../../output/test_rust_engine_template.pdf");
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(output_path, &bytes).unwrap();
    println!("Generated template PDF: {} bytes", bytes.len());
}
