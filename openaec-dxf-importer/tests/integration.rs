//! Integration tests for openaec-dxf-importer.
//!
//! Feeds each fixture through a `CountingSink` and asserts that the
//! expected primitive counts come out. We aim for "non-zero on every
//! fixture" plus a couple of property checks on segments-per-circle
//! / segments-per-arc — the exact numbers are not part of the public
//! API, but they should be stable enough to catch regressions.

use openaec_dxf_importer::{load_dxf_file, DxfSink};
use std::path::PathBuf;

#[derive(Default)]
struct CountingSink {
    segments: usize,
    triangles: usize,
    text_anchors: usize,
    layers: Vec<String>,
    finalized_bbox: Option<[f64; 4]>,
}

impl DxfSink for CountingSink {
    fn emit_segment(
        &mut self,
        _x1: f64,
        _y1: f64,
        _x2: f64,
        _y2: f64,
        _color_argb: u32,
        _layer_idx: u16,
        _entity_idx: u32,
        _dash_kind: u8,
    ) {
        self.segments += 1;
    }

    fn emit_triangle(
        &mut self,
        _x1: f64,
        _y1: f64,
        _x2: f64,
        _y2: f64,
        _x3: f64,
        _y3: f64,
        _color_argb: u32,
        _layer_idx: u16,
        _entity_idx: u32,
    ) {
        self.triangles += 1;
    }

    fn emit_text(
        &mut self,
        _anchor: [f64; 2],
        _height: f64,
        _rotation_rad: f64,
        _text: &str,
        _style_font: Option<&str>,
        _color_argb: u32,
        _layer_idx: u16,
    ) {
        self.text_anchors += 1;
    }

    fn add_layer(&mut self, name: &str, _color_argb: u32, _linetype: &str) -> u16 {
        let idx = self.layers.len() as u16;
        self.layers.push(name.to_string());
        idx
    }

    fn finalize(&mut self, bbox: [f64; 4]) {
        self.finalized_bbox = Some(bbox);
    }
}

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

#[test]
fn line_fixture_emits_segments() {
    let mut sink = CountingSink::default();
    load_dxf_file(&fixture("line_2010.dxf"), &mut sink).expect("load_dxf_file");
    assert!(sink.segments > 0, "expected at least one segment for line fixture");
    assert!(sink.finalized_bbox.is_some(), "finalize must be called");
    let bb = sink.finalized_bbox.unwrap();
    assert!(bb[0].is_finite() && bb[2].is_finite(), "bbox must be finite");
}

#[test]
fn circle_fixture_emits_many_segments() {
    let mut sink = CountingSink::default();
    load_dxf_file(&fixture("circle_2010.dxf"), &mut sink).expect("load_dxf_file");
    // A single CIRCLE tessellates to 64 chord segments. The fixture
    // may carry additional axis-marker LINEs around the circle, so
    // the lower bound is "at least 64".
    assert!(
        sink.segments >= 64,
        "expected >=64 segments for circle fixture (one full 64-chord circle), got {}",
        sink.segments
    );
}

#[test]
fn arc_fixture_emits_segments() {
    let mut sink = CountingSink::default();
    load_dxf_file(&fixture("arc_2010.dxf"), &mut sink).expect("load_dxf_file");
    // A half-arc tessellates to ~32 chords; less if the sweep is
    // smaller. Be lenient — just assert non-zero.
    assert!(
        sink.segments > 0,
        "expected at least one segment for arc fixture"
    );
}

#[test]
fn finalize_always_called() {
    for f in ["line_2010.dxf", "circle_2010.dxf", "arc_2010.dxf"] {
        let mut sink = CountingSink::default();
        load_dxf_file(&fixture(f), &mut sink).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert!(sink.finalized_bbox.is_some(), "finalize missing for {f}");
    }
}
