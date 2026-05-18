//! Per-entity tessellators.
//!
//! Each submodule handles a single DXF entity type. Functions take the
//! parsed `dxf` entity, a per-entity emission context (colour, layer
//! index, entity index, dash kind, linetype), and a mutable `DxfSink`
//! into which they push tessellated primitives.
//!
//! v0.1 wires `LINE`, `CIRCLE`, `ARC`, and the two polyline variants.
//! Other variants live as `pub(crate) fn warn_*` stubs in `walker.rs`
//! so the wiring point stays the entity-type `match` in `walker.rs`.

pub mod arc;
pub mod bbox;
pub mod circle;
pub mod line;
pub mod polyline;

/// Carries per-entity emission state through the tessellator calls.
///
/// Pulled out into its own struct so callsites stay short and the
/// `walker` can build it once per entity.
#[derive(Clone, Copy, Debug)]
pub struct EmitCtx {
    /// Resolved 32-bit ARGB colour for this entity.
    pub color_argb: u32,
    /// Layer index returned earlier by `DxfSink::add_layer`.
    pub layer_idx: u16,
    /// Monotonic per-entity id, allocated by the walker.
    pub entity_idx: u32,
    /// 0=solid, 1=dashed, 2=dotted, 3=dash-dot (v0.1 always emits 0).
    pub dash_kind: u8,
}
