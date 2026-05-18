//! # openaec-dxf-importer
//!
//! Generic, sink-driven importer for the Autodesk DXF (Drawing Exchange
//! Format) text and binary streams.
//!
//! The crate parses a DXF file via the public `dxf` crate, then walks
//! the entity list and converts each entity into one or more
//! tessellated primitives (line segments, triangles, text anchors)
//! that it emits through a user-supplied [`DxfSink`] trait. This lets
//! the same importer feed any consumer:
//!
//! - a 2D Canvas / GPU viewer (Open 2D Studio)
//! - an SVG writer
//! - an IFC bridge
//! - a unit-test harness counting entities
//!
//! ## Provenance
//!
//! The tessellation logic was extracted from the
//! [open-2d-studio](https://github.com/OpenAEC-Foundation/open-2d-studio)
//! `scene_io::load_dxf` pipeline. DXF itself is a public Autodesk
//! interchange format documented in the *AutoCAD DXF Reference* PDF;
//! the implementation is original code written from that reference.
//!
//! ## v0.1 scope
//!
//! Currently wired:
//! - [`LINE`](entities::line)
//! - [`CIRCLE`](entities::circle)
//! - [`ARC`](entities::arc)
//! - [`LWPOLYLINE`](entities::polyline) (incl. per-vertex bulges → arc segments)
//! - [`POLYLINE`](entities::polyline) (legacy 2D, incl. bulges)
//!
//! Deferred to v0.2 (stub branches emit a `tracing::warn!` so the consumer
//! can see what was skipped):
//! - `ELLIPSE`, `SPLINE` (parametric curves)
//! - `INSERT` (block expansion + transform stack)
//! - `HATCH` (pattern + boundary tessellation)
//! - `TEXT`, `MTEXT` (glyph outlines / Hershey fallback)
//! - `DIMENSION` (all subtypes — needs DIMSTYLE table parsing)
//! - `SOLID`, `3DFACE` (filled quads/tris)
//!
//! ## Quick start
//!
//! ```no_run
//! use openaec_dxf_importer::{load_dxf_file, DxfSink};
//!
//! #[derive(Default)]
//! struct CountingSink {
//!     segments: usize,
//!     triangles: usize,
//!     layers: usize,
//! }
//!
//! impl DxfSink for CountingSink {
//!     fn emit_segment(
//!         &mut self,
//!         _x1: f64, _y1: f64, _x2: f64, _y2: f64,
//!         _color_argb: u32, _layer_idx: u16, _entity_idx: u32, _dash_kind: u8,
//!     ) {
//!         self.segments += 1;
//!     }
//!     fn emit_triangle(
//!         &mut self,
//!         _x1: f64, _y1: f64, _x2: f64, _y2: f64, _x3: f64, _y3: f64,
//!         _color_argb: u32, _layer_idx: u16, _entity_idx: u32,
//!     ) {
//!         self.triangles += 1;
//!     }
//!     fn emit_text(
//!         &mut self,
//!         _anchor: [f64; 2], _height: f64, _rotation_rad: f64,
//!         _text: &str, _style_font: Option<&str>,
//!         _color_argb: u32, _layer_idx: u16,
//!     ) {}
//!     fn add_layer(&mut self, _name: &str, _color_argb: u32, _linetype: &str) -> u16 {
//!         let idx = self.layers as u16;
//!         self.layers += 1;
//!         idx
//!     }
//!     fn finalize(&mut self, _bbox: [f64; 4]) {}
//! }
//!
//! let mut sink = CountingSink::default();
//! load_dxf_file(std::path::Path::new("drawing.dxf"), &mut sink).unwrap();
//! println!("{} segments, {} triangles", sink.segments, sink.triangles);
//! ```
//!
//! ## License
//!
//! MIT — see the workspace `LICENSE` file.

use std::path::Path;
use thiserror::Error;

pub mod entities;
mod walker;

/// Errors produced by [`load_dxf_file`] / [`load_dxf_str`].
#[derive(Debug, Error)]
pub enum DxfError {
    /// I/O error while reading the source file.
    #[error("I/O error reading DXF: {0}")]
    Io(#[from] std::io::Error),

    /// The underlying `dxf` crate failed to parse the stream.
    #[error("DXF parse error: {0}")]
    Parse(String),
}

impl From<dxf::DxfError> for DxfError {
    fn from(e: dxf::DxfError) -> Self {
        DxfError::Parse(format!("{e}"))
    }
}

/// Consumer-supplied output trait for the tessellated DXF stream.
///
/// The importer calls these methods as it walks the parsed DXF
/// drawing. Implementations decide where the emitted geometry goes —
/// a GPU buffer, an SVG file, a database row, an entity count.
///
/// Implementations must be cheap to call: a typical sheet emits tens
/// of thousands of segments per second. Buffer / batch on your side if
/// you need throughput.
///
/// ### Coordinates
/// All emitted coordinates are in **DXF world units** (millimetres,
/// metres, inches — whatever the source file's `$INSUNITS` says; the
/// importer does not normalise). The y-axis is **cartesian positive
/// up** (DXF native; flip on the consumer side if you target a screen
/// space with y-down).
///
/// ### Colour
/// `color_argb` is a 32-bit ARGB value with `0xFF` alpha for opaque,
/// resolved from the entity's true-color override (if any), its ACI
/// index, or its layer's ACI. Consumers should NOT re-do the
/// BYLAYER/BYBLOCK fallback chain; it's already done.
///
/// ### Layer indexing
/// Layer identifiers are passed as small `u16` indices. The importer
/// allocates them on first sight via [`DxfSink::add_layer`] — the
/// sink decides what `u16` to hand back, but the same `name` MUST
/// always map to the same index.
pub trait DxfSink {
    /// Emit a line segment from `(x1, y1)` to `(x2, y2)`.
    ///
    /// - `color_argb` — 32-bit ARGB colour, alpha in the top byte.
    /// - `layer_idx` — index returned by an earlier [`add_layer`](Self::add_layer) call.
    /// - `entity_idx` — opaque, monotonically increasing per source
    ///   entity. Sink implementations may use it to group segments
    ///   that belong to the same source entity (selection, hover).
    /// - `dash_kind` — `0 = solid`, `1 = dashed`, `2 = dotted`,
    ///   `3 = dash-dot`. Solid is the safe default if the importer
    ///   couldn't classify the linetype.
    fn emit_segment(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color_argb: u32,
        layer_idx: u16,
        entity_idx: u32,
        dash_kind: u8,
    );

    /// Emit a filled triangle (CCW winding).
    ///
    /// Used for SOLID, 3DFACE, and HATCH fills. v0.1 does not emit
    /// any triangles; this method is here for forward compatibility.
    fn emit_triangle(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x3: f64,
        y3: f64,
        color_argb: u32,
        layer_idx: u16,
        entity_idx: u32,
    );

    /// Emit a text anchor.
    ///
    /// Raw text is passed through unchanged (no glyph tessellation,
    /// no DXF escape decoding). `style_font` is the resolved primary
    /// font filename from the source STYLE table, e.g. `"swissc.ttf"`
    /// or `"romans"`; `None` if the STYLE was missing.
    ///
    /// v0.1 does not call this method; stubbed for forward compat.
    fn emit_text(
        &mut self,
        anchor: [f64; 2],
        height: f64,
        rotation_rad: f64,
        text: &str,
        style_font: Option<&str>,
        color_argb: u32,
        layer_idx: u16,
    );

    /// Register a layer with the sink and return its assigned index.
    ///
    /// Called by the importer at the start of the entity walk for
    /// every layer present in the LAYERS table. The sink decides
    /// what `u16` to hand back; the importer will then use that
    /// index in subsequent [`emit_segment`](Self::emit_segment) etc. calls.
    fn add_layer(&mut self, name: &str, color_argb: u32, linetype: &str) -> u16;

    /// Called once after the last emit, with the bounding box of all
    /// emitted segments + triangles in `[xmin, ymin, xmax, ymax]`
    /// order. Useful for camera framing.
    fn finalize(&mut self, bbox: [f64; 4]);
}

/// Load a DXF file from disk and feed it through `sink`.
///
/// Reads + parses synchronously. Streaming is not supported by the
/// underlying `dxf` crate.
pub fn load_dxf_file(path: &Path, sink: &mut dyn DxfSink) -> Result<(), DxfError> {
    let drawing = dxf::Drawing::load_file(path)?;
    walker::walk(&drawing, sink);
    Ok(())
}

/// Parse a DXF string in-memory and feed it through `sink`.
///
/// Convenience for tests and clipboard-driven flows.
pub fn load_dxf_str(content: &str, sink: &mut dyn DxfSink) -> Result<(), DxfError> {
    let drawing = dxf::Drawing::load(&mut content.as_bytes())?;
    walker::walk(&drawing, sink);
    Ok(())
}
