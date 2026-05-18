//! DXF `LWPOLYLINE` and `POLYLINE` entities.
//!
//! Both store a sequence of 2D vertices with optional per-vertex
//! `bulge` (group code 42), which converts the outgoing chord into a
//! tangent arc. The two entity types differ only in the wire
//! representation: `LWPOLYLINE` is the compact post-R13 form,
//! `POLYLINE` is the heavy legacy form with a child `VERTEX` per
//! vertex.
//!
//! References:
//! - AutoCAD DXF Reference, AcDbPolyline (`100 AcDbPolyline`), group
//!   codes `90` (vertex count), `70` (flags, bit 1 = closed), `10/20`
//!   (xy per vertex), `42` (bulge per vertex).
//! - ODA OpenDesign Spec §19.4.87 / §20.4.85 — `bulge = tan(sweep / 4)`.

use crate::DxfSink;

use super::{bbox, EmitCtx};

/// Tessellate an `LWPOLYLINE` (compact form).
pub fn tessellate_lw(
    pl: &dxf::entities::LwPolyline,
    ctx: EmitCtx,
    sink: &mut dyn DxfSink,
    bb: &mut [f64; 4],
) {
    let verts: Vec<[f64; 2]> = pl.vertices.iter().map(|v| [v.x, v.y]).collect();
    let bulges: Vec<f64> = pl.vertices.iter().map(|v| v.bulge).collect();
    let closed = pl.get_is_closed() && verts.len() > 2;
    emit(&verts, &bulges, closed, ctx, sink, bb);
}

/// Tessellate a legacy `POLYLINE` (`VERTEX`-child form).
pub fn tessellate_legacy(
    pl: &dxf::entities::Polyline,
    ctx: EmitCtx,
    sink: &mut dyn DxfSink,
    bb: &mut [f64; 4],
) {
    let mut verts: Vec<[f64; 2]> = Vec::new();
    let mut bulges: Vec<f64> = Vec::new();
    for v in pl.vertices() {
        verts.push([v.location.x, v.location.y]);
        bulges.push(v.bulge);
    }
    let closed = pl.get_is_closed() && verts.len() > 2;
    emit(&verts, &bulges, closed, ctx, sink, bb);
}

/// Shared emit loop — handles bulge expansion uniformly across both
/// polyline types.
fn emit(
    verts: &[[f64; 2]],
    bulges: &[f64],
    closed: bool,
    ctx: EmitCtx,
    sink: &mut dyn DxfSink,
    bb: &mut [f64; 4],
) {
    if verts.len() < 2 {
        return;
    }
    let n = verts.len();
    let last_idx = if closed { n } else { n - 1 };
    for i in 0..last_idx {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let bulge = bulges.get(i).copied().unwrap_or(0.0);
        for (p1, p2) in expand_bulge(a, b, bulge) {
            sink.emit_segment(
                p1[0],
                p1[1],
                p2[0],
                p2[1],
                ctx.color_argb,
                ctx.layer_idx,
                ctx.entity_idx,
                ctx.dash_kind,
            );
            bbox::expand(bb, p2[0], p2[1]);
        }
        bbox::expand(bb, a[0], a[1]);
    }
}

/// Expand a single chord `a → b` with bulge `bulge` into one straight
/// segment (if bulge is zero) or a fan of chord-arcs.
///
/// `bulge = tan(sweep / 4)`, signed by direction (positive = CCW).
fn expand_bulge(a: [f64; 2], b: [f64; 2], bulge: f64) -> Vec<([f64; 2], [f64; 2])> {
    let mut out: Vec<([f64; 2], [f64; 2])> = Vec::new();
    if bulge.abs() < 1e-12 {
        out.push((a, b));
        return out;
    }
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let chord_len = (dx * dx + dy * dy).sqrt();
    if chord_len < 1e-12 {
        out.push((a, b));
        return out;
    }
    let abs_sweep = 4.0 * bulge.abs().atan();
    let sweep = if bulge > 0.0 { abs_sweep } else { -abs_sweep };
    let half_sweep = abs_sweep * 0.5;
    let sin_half = half_sweep.sin();
    if sin_half.abs() < 1e-12 {
        out.push((a, b));
        return out;
    }
    let r = chord_len / (2.0 * sin_half);
    let ux = dx / chord_len;
    let uy = dy / chord_len;
    let px = -uy;
    let py = ux;
    let h_offset = r * half_sweep.cos();
    let sign = if bulge > 0.0 { 1.0 } else { -1.0 };
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let center = [mid[0] + px * h_offset * sign, mid[1] + py * h_offset * sign];
    let start_ang = (a[1] - center[1]).atan2(a[0] - center[0]);
    let seg_count = ((abs_sweep / std::f64::consts::TAU * 32.0).ceil() as usize)
        .max(4)
        .min(128);
    let r_abs = r.abs();
    let mut prev = a;
    for k in 1..=seg_count {
        let t = start_ang + sweep * (k as f64) / (seg_count as f64);
        let cur = if k == seg_count {
            b
        } else {
            [center[0] + r_abs * t.cos(), center[1] + r_abs * t.sin()]
        };
        out.push((prev, cur));
        prev = cur;
    }
    out
}
