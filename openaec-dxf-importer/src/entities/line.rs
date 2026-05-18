//! DXF `LINE` entity (group code 0 = "LINE").
//!
//! Single straight segment between two 3D points. The Z component is
//! dropped (the importer is 2D-only).
//!
//! Reference: AutoCAD DXF Reference, AcDbLine subclass marker `100
//! AcDbLine`, group codes `10/20/30` (start) and `11/21/31` (end).

use crate::DxfSink;

use super::{bbox, EmitCtx};

/// Tessellate a DXF `Line` into a single segment.
pub fn tessellate(line: &dxf::entities::Line, ctx: EmitCtx, sink: &mut dyn DxfSink, bb: &mut [f64; 4]) {
    let x1 = line.p1.x;
    let y1 = line.p1.y;
    let x2 = line.p2.x;
    let y2 = line.p2.y;
    sink.emit_segment(
        x1,
        y1,
        x2,
        y2,
        ctx.color_argb,
        ctx.layer_idx,
        ctx.entity_idx,
        ctx.dash_kind,
    );
    bbox::expand(bb, x1, y1);
    bbox::expand(bb, x2, y2);
}
