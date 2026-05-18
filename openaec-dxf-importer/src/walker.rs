//! Entity walker — drives the per-entity tessellators.
//!
//! Two-pass design:
//! 1. Register every layer with the sink (resolves layer→ARGB
//!    palette + linetype name).
//! 2. Walk `drawing.entities()` and dispatch each entity to the
//!    appropriate `entities::*::tessellate` function.
//!
//! Entity types not yet wired in v0.1 emit a `tracing::warn!` once
//! per type so the consumer can see them in the log; they don't fail
//! the import.

use std::collections::HashMap;

use crate::entities::{self, EmitCtx};
use crate::DxfSink;

/// AutoCAD Color Index → ARGB palette.
///
/// Indices 0 (BYBLOCK) and 256 (BYLAYER) return `0`; the caller
/// resolves those by falling back to the layer colour. The 10-249
/// band is a structured HSV wheel — 24 hue groups of 10 shade
/// variants. See [`color_palette`] for the full lookup.
fn aci_to_argb(aci: i16) -> u32 {
    match aci {
        0 | 256 => 0,
        1 => 0xFF0000FF,
        2 => 0xFF00FFFF,
        3 => 0xFF00FF00,
        4 => 0xFFFFFF00,
        5 => 0xFFFF0000,
        6 => 0xFFFF00FF,
        7 => 0xFFFFFFFF,
        8 => 0xFF555555,
        9 => 0xFFAAAAAA,
        n if n > 0 && n <= 255 => {
            // 24-hue x 10-shade structured palette. Matches the
            // open-2d-studio reference renderer.
            let group = (n as i32 - 10).max(0) / 10;
            let variant = ((n as i32 - 10).max(0) % 10) as i32;
            let hue_deg = ((group * 15) % 360) as f32;
            let (v, s) = match variant {
                0 => (1.00_f32, 1.00_f32),
                2 => (0.85_f32, 1.00_f32),
                4 => (0.70_f32, 1.00_f32),
                6 => (0.55_f32, 1.00_f32),
                8 => (0.40_f32, 1.00_f32),
                1 => (1.00_f32, 0.60_f32),
                3 => (1.00_f32, 0.40_f32),
                5 => (0.85_f32, 0.50_f32),
                7 => (0.70_f32, 0.50_f32),
                _ => (0.55_f32, 0.50_f32),
            };
            let h = hue_deg / 60.0;
            let c = v * s;
            let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
            let (r, g, b) = match h as u32 {
                0 => (c, x, 0.0),
                1 => (x, c, 0.0),
                2 => (0.0, c, x),
                3 => (0.0, x, c),
                4 => (x, 0.0, c),
                _ => (c, 0.0, x),
            };
            let m = v - c;
            let ri = ((r + m) * 255.0) as u32 & 0xFF;
            let gi = ((g + m) * 255.0) as u32 & 0xFF;
            let bi = ((b + m) * 255.0) as u32 & 0xFF;
            0xFF000000 | (bi << 16) | (gi << 8) | ri
        }
        _ => 0,
    }
}

/// Walk the parsed DXF drawing and emit tessellated primitives.
pub(crate) fn walk(drawing: &dxf::Drawing, sink: &mut dyn DxfSink) {
    // 1. Register every layer up front so the sink can allocate
    //    indices and so we can resolve BYLAYER fallback later.
    let mut layer_color: HashMap<String, u32> = HashMap::new();
    let mut layer_idx: HashMap<String, u16> = HashMap::new();
    for l in drawing.layers() {
        let argb = match l.color.index() {
            Some(i) => aci_to_argb(i as i16),
            None => 0xFFFFFFFF,
        };
        let key = l.name.to_ascii_uppercase();
        let idx = sink.add_layer(&l.name, argb, &l.line_type_name);
        layer_color.insert(key.clone(), argb);
        layer_idx.insert(key, idx);
    }

    // 2. Track once-per-type deferred warnings so the log stays
    //    readable even on multi-thousand-entity sheets.
    let mut warned = WarnFlags::default();

    let mut bbox = entities::bbox::INIT;
    let mut entity_id: u32 = 0;

    for ent in drawing.entities() {
        let layer_key = ent.common.layer.to_ascii_uppercase();
        let layer_argb = layer_color.get(&layer_key).copied().unwrap_or(0xFFFFFFFF);
        let lidx = layer_idx.get(&layer_key).copied().unwrap_or(0);

        // Colour resolution priority (matches open-2d-studio):
        //   1. true-color override (color_24_bit != 0).
        //   2. ACI index from entity.common.color (1..255).
        //   3. BYLAYER → layer_color lookup.
        let color: u32 = {
            let tc = ent.common.color_24_bit;
            if tc != 0 {
                let r = ((tc >> 16) & 0xFF) as u32;
                let g = ((tc >> 8) & 0xFF) as u32;
                let b = (tc & 0xFF) as u32;
                0xFF000000 | (b << 16) | (g << 8) | r
            } else if let Some(idx) = ent.common.color.index() {
                let argb = aci_to_argb(idx as i16);
                if argb == 0 {
                    layer_argb
                } else {
                    argb
                }
            } else {
                layer_argb
            }
        };

        let ctx = EmitCtx {
            color_argb: color,
            layer_idx: lidx,
            entity_idx: entity_id,
            dash_kind: 0,
        };

        use dxf::entities::EntityType as E;
        match &ent.specific {
            E::Line(l) => entities::line::tessellate(l, ctx, sink, &mut bbox),
            E::Circle(c) => entities::circle::tessellate(c, ctx, sink, &mut bbox),
            E::Arc(a) => entities::arc::tessellate(a, ctx, sink, &mut bbox),
            E::LwPolyline(pl) => entities::polyline::tessellate_lw(pl, ctx, sink, &mut bbox),
            E::Polyline(pl) => entities::polyline::tessellate_legacy(pl, ctx, sink, &mut bbox),

            // ---- Deferred to v0.2 ------------------------------------
            E::Ellipse(_) => {
                if !warned.ellipse {
                    warned.ellipse = true;
                    tracing::warn!("openaec-dxf-importer v0.1: ELLIPSE entity ignored (deferred to v0.2)");
                }
            }
            E::Spline(_) => {
                if !warned.spline {
                    warned.spline = true;
                    tracing::warn!("openaec-dxf-importer v0.1: SPLINE entity ignored (deferred to v0.2)");
                }
            }
            E::Insert(_) => {
                if !warned.insert {
                    warned.insert = true;
                    tracing::warn!("openaec-dxf-importer v0.1: INSERT block expansion ignored (deferred to v0.2)");
                }
            }
            E::Solid(_) => {
                if !warned.solid {
                    warned.solid = true;
                    tracing::warn!("openaec-dxf-importer v0.1: SOLID entity ignored (deferred to v0.2)");
                }
            }
            E::Face3D(_) => {
                if !warned.face3d {
                    warned.face3d = true;
                    tracing::warn!("openaec-dxf-importer v0.1: 3DFACE entity ignored (deferred to v0.2)");
                }
            }
            E::Text(_) => {
                if !warned.text {
                    warned.text = true;
                    tracing::warn!("openaec-dxf-importer v0.1: TEXT entity ignored (deferred to v0.2)");
                }
            }
            E::MText(_) => {
                if !warned.mtext {
                    warned.mtext = true;
                    tracing::warn!("openaec-dxf-importer v0.1: MTEXT entity ignored (deferred to v0.2)");
                }
            }
            E::RotatedDimension(_)
            | E::AngularThreePointDimension(_)
            | E::DiameterDimension(_)
            | E::RadialDimension(_)
            | E::OrdinateDimension(_) => {
                if !warned.dimension {
                    warned.dimension = true;
                    tracing::warn!("openaec-dxf-importer v0.1: DIMENSION entity ignored (deferred to v0.2)");
                }
            }
            // HATCH is dropped silently by dxf-0.5 itself in many cases —
            // a clean re-parse pass would be needed and that's part of
            // the v0.2 work. Don't warn here; it would fire on
            // every file regardless of HATCH presence.

            _ => {
                if !warned.other {
                    warned.other = true;
                    tracing::warn!(
                        "openaec-dxf-importer v0.1: entity type {:?} ignored (deferred to v0.2)",
                        std::mem::discriminant(&ent.specific),
                    );
                }
            }
        }

        entity_id = entity_id.wrapping_add(1);
    }

    // Empty drawings: clamp the sentinel bbox to zeros so the consumer
    // doesn't have to special-case infinities.
    if bbox[0].is_infinite() {
        bbox = [0.0, 0.0, 0.0, 0.0];
    }
    sink.finalize(bbox);
}

#[derive(Default)]
struct WarnFlags {
    ellipse: bool,
    spline: bool,
    insert: bool,
    solid: bool,
    face3d: bool,
    text: bool,
    mtext: bool,
    dimension: bool,
    other: bool,
}
