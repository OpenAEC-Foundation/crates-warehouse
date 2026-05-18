//! DXF `ARC` entity (group code 0 = "ARC").
//!
//! Reference: AutoCAD DXF Reference, AcDbArc subclass marker `100
//! AcDbArc`, group codes `10/20/30` (centre), `40` (radius), `50`
//! (start angle, deg), `51` (end angle, deg).
//!
//! Arc sweeps CCW from start to end. If `end < start`, add 2π to end
//! so the sweep stays positive. Chord count is proportional to the
//! sweep (64 chords for a full turn), min 4.

use crate::DxfSink;

use super::{bbox, EmitCtx};

/// Tessellate a DXF `Arc` into a fan of chord segments.
pub fn tessellate(a: &dxf::entities::Arc, ctx: EmitCtx, sink: &mut dyn DxfSink, bb: &mut [f64; 4]) {
    let cx = a.center.x;
    let cy = a.center.y;
    let r = a.radius;
    let s = a.start_angle.to_radians();
    let e_raw = a.end_angle.to_radians();
    let e = if e_raw < s {
        e_raw + std::f64::consts::TAU
    } else {
        e_raw
    };
    let sweep = e - s;
    let n = ((sweep / std::f64::consts::TAU * 64.0).ceil() as usize).max(4);

    let mut prev_x = cx + r * s.cos();
    let mut prev_y = cy + r * s.sin();
    bbox::expand(bb, prev_x, prev_y);
    for i in 1..=n {
        let t = s + sweep * (i as f64) / (n as f64);
        let cur_x = cx + r * t.cos();
        let cur_y = cy + r * t.sin();
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
