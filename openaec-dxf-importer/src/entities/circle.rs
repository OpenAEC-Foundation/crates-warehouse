//! DXF `CIRCLE` entity (group code 0 = "CIRCLE").
//!
//! Reference: AutoCAD DXF Reference, AcDbCircle subclass marker `100
//! AcDbCircle`, group codes `10/20/30` (centre) and `40` (radius).
//!
//! Tessellated as 64 chords around the full sweep — good enough for
//! print-resolution output; the consumer can re-tessellate at a
//! finer resolution if a higher-quality preview is needed.

use crate::DxfSink;

use super::{bbox, EmitCtx};

/// Chord count used to approximate a full circle. Matches the
/// open-2d-studio reference renderer.
const CHORDS: usize = 64;

/// Tessellate a DXF `Circle` into a fan of chord segments.
pub fn tessellate(c: &dxf::entities::Circle, ctx: EmitCtx, sink: &mut dyn DxfSink, bb: &mut [f64; 4]) {
    let cx = c.center.x;
    let cy = c.center.y;
    let r = c.radius;
    let mut prev_x = cx + r;
    let mut prev_y = cy;
    bbox::expand(bb, prev_x, prev_y);
    for i in 1..=CHORDS {
        let a = (i as f64) / (CHORDS as f64) * std::f64::consts::TAU;
        let cur_x = cx + r * a.cos();
        let cur_y = cy + r * a.sin();
        sink.emit_segment(
            prev_x,
            prev_y,
            cur_x,
            cur_y,
            ctx.color_argb,
            ctx.layer_idx,
            ctx.entity_idx,
            ctx.dash_kind,
        );
        bbox::expand(bb, cur_x, cur_y);
        prev_x = cur_x;
        prev_y = cur_y;
    }
}
