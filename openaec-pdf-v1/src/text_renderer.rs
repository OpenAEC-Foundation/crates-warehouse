use crate::font_parser::OutlineCommand;
use crate::fonts::{FontEntry, FontRegistry};
use crate::draw_commands::DrawCommandBuffer;
use crate::renderer::SkiaRenderer;
use crate::graphics_state::GraphicsStateStack;

/// Per-render glyph path cache keyed by (font ObjectId, glyph_id). Speed
/// iter-23 cache: building a `tiny_skia::Path` from `OutlineCommand`s is
/// the largest single cost in text-heavy pages (~70% of render time on
/// Zware vector PDF p3/p5). The cache lives for one page render so the
/// same glyph outline is materialised at most once even when it appears
/// thousands of times. The cached Path is in glyph-units; the per-call
/// `concat_matrix(sh*tm[0], …)` then maps it to user space, so a single
/// cached entry is reusable at every position and every scale.
pub type GlyphPathCache = std::collections::HashMap<(lopdf::ObjectId, u32), tiny_skia::Path>;

/// Resolve the advance width for a character code, in text-space units
/// (i.e. fraction of `font_size`).
///
/// PDF spec §9.4.4 / §9.7.3 / §9.7.4.3: the authoritative advance width for
/// each character code is read from the PDF font dictionary's `/Widths`
/// (simple fonts) or `/W` (Type0/CID fonts) array, in 1/1000-em units. The
/// embedded font program's internal advance widths can differ — especially
/// for subsetted fonts where the PDF preserves the original widths but the
/// embedded `hmtx` table may have been pruned or replaced. Using the PDF
/// widths is required to match other PDF renderers' line-breaking and
/// glyph positioning.
///
/// Falls back to the embedded font's `outline.advance_width / units_per_em`
/// when the PDF dictionary has no entry for the code (and no /MissingWidth
/// or /DW default), which preserves correct rendering for fonts whose
/// width tables happen to be omitted (rare).
#[inline]
fn pdf_advance_width(
    font_entry: &FontEntry,
    code: u32,
    fallback_advance_em: f32,
) -> f32 {
    if let Some(&w) = font_entry.widths.get(&code) {
        return w / 1000.0;
    }
    // /MissingWidth (simple) or /DW (Type0). Treat 0.0 as "not specified" so
    // the embedded fallback runs — most PDFs that omit a code in /Widths
    // genuinely want the embedded font's width, and a default of 0 would
    // collapse the glyph onto the previous one.
    if font_entry.default_width > 0.0 {
        return font_entry.default_width / 1000.0;
    }
    fallback_advance_em
}

/// Render a text string as vector glyph outlines.
///
/// PDF text rendering follows the spec §9.4.4 exactly:
///
///   Trm = [Tfs×Th  0      0] × [Tm] × [CTM]
///         [0       Tfs    0]
///         [0       Trise  1]
///
/// CTM is already applied via the graphics state transform commands in the draw buffer.
/// So we compute: Trm = [Tfs×Th 0; 0 Tfs; 0 Trise] × Tm
///
/// For each glyph:
/// 1. Compute rendering position from Trm (includes rise offset)
/// 2. Scale glyph outlines by 1/units_per_em (glyph coords → text space)
/// 3. Apply Trm rotation/scale components
/// 4. Emit path commands + fill
/// 5. Advance Tm: tx = (w0 × Tfs + Tc + Tw) × Th; Tm = [1 0 0 1 tx 0] × Tm
///
/// Parameters:
/// - `text_bytes`: raw bytes from the Tj/TJ string
/// - `font_entry`: the resolved font with parsed glyph data
/// - `font_size`: Tfs (from Tf operator)
/// - `horizontal_scaling`: Th (from Tz operator, 1.0 = 100%)
/// - `char_spacing`: Tc (from Tc operator)
/// - `word_spacing`: Tw (from Tw operator, applied to space char code 32)
/// - `rise`: Trise (from Ts operator)
/// - `tm`: text matrix [a b c d e f] — MUTATED with character advances
/// - `fill_rgba`: fill color
/// - `buf`: draw command buffer
pub fn render_text_glyphs(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: u32,
    buf: &mut DrawCommandBuffer,
) {
    let parsed = match &font_entry.parsed {
        Some(p) => p,
        None => return,
    };

    let upm = parsed.units_per_em as f32;

    for &byte in text_bytes {
        let glyph_id = match FontRegistry::char_to_glyph_id(font_entry, byte) {
            Some(id) => id,
            None => {
                // Unknown glyph — still advance by estimated width
                let w0 = 0.5; // fallback: half em
                let tw = if byte == 32 { word_spacing } else { 0.0 };
                let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
                tm[4] += tx * tm[0];
                tm[5] += tx * tm[1];
                continue;
            }
        };

        if let Some(outline) = parsed.glyphs.get(&glyph_id) {
            if !outline.commands.is_empty() {
                // Compute Trm components (without CTM — already in graphics state):
                // Trm_a = Tfs × Th × Tm[0] / upm
                // Trm_b = Tfs × Th × Tm[1] / upm
                // Trm_c = Tfs × Tm[2] / upm
                // Trm_d = Tfs × Tm[3] / upm
                // Trm_e = Trise × Tm[2] + Tm[4]  (render position x)
                // Trm_f = Trise × Tm[3] + Tm[5]  (render position y)
                let s = font_size / upm;
                let sh = s * horizontal_scaling;
                let gx = rise * tm[2] + tm[4];
                let gy = rise * tm[3] + tm[5];

                buf.save_state();
                buf.transform(sh * tm[0], sh * tm[1], s * tm[2], s * tm[3], gx, gy);
                buf.begin_path();
                for cmd in &outline.commands {
                    match cmd {
                        OutlineCommand::MoveTo(x, y) => buf.move_to(*x, *y),
                        OutlineCommand::LineTo(x, y) => buf.line_to(*x, *y),
                        OutlineCommand::CubicTo(x1, y1, x2, y2, x, y) => {
                            buf.cubic_to(*x1, *y1, *x2, *y2, *x, *y)
                        }
                        OutlineCommand::Close => buf.close_path(),
                    }
                }
                buf.set_fill(fill_rgba);
                buf.fill();
                buf.restore_state();
            }

            // Advance text matrix per PDF spec §9.4.4:
            // w0 = advance_width / units_per_em (displacement in text space)
            // tx = (w0 × Tfs + Tc + Tw) × Th
            // Tm = [1 0 0 1 tx 0] × Tm
            //
            // Width source: PDF /Widths array first, embedded font hmtx as
            // fallback (see pdf_advance_width comment).
            let w0 = pdf_advance_width(
                font_entry,
                byte as u32,
                outline.advance_width / upm,
            );
            let tw = if byte == 32 { word_spacing } else { 0.0 };
            let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
            tm[4] += tx * tm[0];
            tm[5] += tx * tm[1];
        }
    }
}

/// Render CID-encoded text (Type0/CID fonts) — 2 bytes per character.
/// Used for Identity-H/Identity-V encoded fonts where each 2-byte value is a CID.
pub fn render_cid_text_glyphs(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: u32,
    buf: &mut DrawCommandBuffer,
) {
    let parsed = match &font_entry.parsed {
        Some(p) => p,
        None => return,
    };

    let upm = parsed.units_per_em as f32;

    // Process 2-byte character codes (big-endian)
    let mut i = 0;
    while i + 1 < text_bytes.len() {
        let cid = u16::from_be_bytes([text_bytes[i], text_bytes[i + 1]]);
        i += 2;

        let glyph_id = match FontRegistry::cid_to_glyph_id(font_entry, cid) {
            Some(id) => id,
            None => {
                // Unknown glyph — advance by estimated width
                let w0 = 0.5;
                let tx = (w0 * font_size + char_spacing) * horizontal_scaling;
                tm[4] += tx * tm[0];
                tm[5] += tx * tm[1];
                continue;
            }
        };

        if let Some(outline) = parsed.glyphs.get(&glyph_id) {
            if !outline.commands.is_empty() {
                let s = font_size / upm;
                let sh = s * horizontal_scaling;
                let gx = rise * tm[2] + tm[4];
                let gy = rise * tm[3] + tm[5];

                buf.save_state();
                buf.transform(sh * tm[0], sh * tm[1], s * tm[2], s * tm[3], gx, gy);
                buf.begin_path();
                for cmd in &outline.commands {
                    match cmd {
                        OutlineCommand::MoveTo(x, y) => buf.move_to(*x, *y),
                        OutlineCommand::LineTo(x, y) => buf.line_to(*x, *y),
                        OutlineCommand::CubicTo(x1, y1, x2, y2, x, y) => {
                            buf.cubic_to(*x1, *y1, *x2, *y2, *x, *y)
                        }
                        OutlineCommand::Close => buf.close_path(),
                    }
                }
                buf.set_fill(fill_rgba);
                buf.fill();
                buf.restore_state();
            }

            // Width source: PDF /W array first (keyed by CID), embedded font
            // hmtx as fallback (see pdf_advance_width comment).
            let w0 = pdf_advance_width(
                font_entry,
                cid as u32,
                outline.advance_width / upm,
            );
            // CID space char is typically U+0020 = CID 3 (font-dependent)
            let tw = if cid == 3 || cid == 32 { word_spacing } else { 0.0 };
            let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
            tm[4] += tx * tm[0];
            tm[5] += tx * tm[1];
        }
    }
}

/// Render simple-encoded (1-byte char codes) text directly into a SkiaRenderer.
///
/// Mirrors `render_text_glyphs` but emits paths through the renderer +
/// graphics-state stack rather than into a binary draw-command buffer. Used
/// by `Interpreter::execute_internal` so server-side rasterization (e.g. the
/// MCP `screenshot_page` tool) actually paints text glyphs instead of
/// silently dropping them.
///
/// Geometry follows PDF spec §9.4.4: per glyph we save state, pre-concat
/// `Trm = [Tfs×Th 0; 0 Tfs; 0 Trise] × Tm` onto the CTM, fill the outline,
/// then advance Tm by `(w0 × Tfs + Tc + Tw) × Th`.
pub fn render_text_glyphs_skia(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: (u8, u8, u8, u8),
    renderer: &mut SkiaRenderer,
    state: &mut GraphicsStateStack,
    glyph_cache: Option<(lopdf::ObjectId, &mut GlyphPathCache)>,
) {
    render_text_glyphs_skia_with_mode(
        text_bytes, font_entry, font_size, horizontal_scaling,
        char_spacing, word_spacing, rise, tm, fill_rgba,
        renderer, state, glyph_cache, 0,
    );
}

/// PDF 1.7 §9.3.6 text rendering mode dispatch (`Tr` operator).
/// `render_mode`:
///   0 = fill (default)
///   1 = stroke
///   2 = fill, then stroke (synthetic-bold idiom)
///   3 = invisible (skip painting; advance only — for selection overlays)
///   4-7 = same as 0-3 plus add to clipping path (clipping side-effect not
///         implemented here — the visible paint matches mode 0-3).
///
/// The fill colour is `fill_rgba`; the stroke colour and line width come from
/// `state.current.stroke_color` / `state.current.line_width`. Both colours
/// already include the ExtGState `/ca`/`/CA` opacity from `gs.effective_*`.
pub fn render_text_glyphs_skia_with_mode(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: (u8, u8, u8, u8),
    renderer: &mut SkiaRenderer,
    state: &mut GraphicsStateStack,
    glyph_cache: Option<(lopdf::ObjectId, &mut GlyphPathCache)>,
    render_mode: u8,
) {
    let parsed = match &font_entry.parsed {
        Some(p) => p,
        None => return,
    };

    let upm = parsed.units_per_em as f32;
    let (font_id_opt, mut cache_opt) = match glyph_cache {
        Some((id, cache)) => (Some(id), Some(cache)),
        None => (None, None),
    };

    // Mode 3 / 7 = invisible — advance only, no painting.
    let do_fill = matches!(render_mode, 0 | 2 | 4 | 6);
    let do_stroke = matches!(render_mode, 1 | 2 | 5 | 6);

    for &byte in text_bytes {
        let glyph_id = match FontRegistry::char_to_glyph_id(font_entry, byte) {
            Some(id) => id,
            None => {
                // Unknown glyph — still advance by estimated width
                let w0 = 0.5;
                let tw = if byte == 32 { word_spacing } else { 0.0 };
                let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
                tm[4] += tx * tm[0];
                tm[5] += tx * tm[1];
                continue;
            }
        };

        if let Some(outline) = parsed.glyphs.get(&glyph_id) {
            if !outline.commands.is_empty() && (do_fill || do_stroke) {
                let s = font_size / upm;
                let sh = s * horizontal_scaling;
                let gx = rise * tm[2] + tm[4];
                let gy = rise * tm[3] + tm[5];
                let (sgx, sgy) = snap_glyph_origin(gx, gy, &state.current.ctm);

                // Build (or fetch) the cached glyph Path. Keyed by
                // (font ObjectId, glyph_id) — both stable for the lifetime
                // of the document so the cached Path is reusable across
                // pages within the same render. We re-build the Path if
                // there's no cache (e.g. inline font dict with no ObjectId).
                let path_opt = if let (Some(font_id), Some(cache)) = (font_id_opt, cache_opt.as_deref_mut()) {
                    Some(cache.entry((font_id, glyph_id as u32))
                        .or_insert_with(|| build_glyph_path(&outline.commands))
                        .clone())
                } else {
                    build_glyph_path_opt(&outline.commands)
                };

                if let Some(path) = path_opt {
                    // Speed iter-23: avoid the full state.save() / restore()
                    // round-trip — that path clones the entire GraphicsState
                    // including the clip-mask bitmap (≈ width × height bytes,
                    // multiple MB on 2000-pixel renders). For thousands of
                    // glyphs per page this dominated the render cost.
                    // Instead, save just the CTM + fill_color + stroke_color +
                    // line_width, mutate, paint, and restore by hand.
                    let saved_ctm = state.current.ctm;
                    let saved_fill = state.current.fill_color;
                    let saved_stroke = state.current.stroke_color;
                    let saved_line_width = state.current.line_width;
                    let saved_dash = state.current.dash_array.clone();
                    state.current.fill_color = fill_rgba;
                    state.current.ctm = state.current.ctm.pre_concat(
                        tiny_skia::Transform::from_row(
                            sh * tm[0], sh * tm[1], s * tm[2], s * tm[3], sgx, sgy,
                        ),
                    );
                    if do_fill {
                        renderer.fill_cached_path(&path, &state.current, false);
                    }
                    if do_stroke {
                        // The CTM was just pre-concated with a per-glyph scale
                        // `s = font_size / upm`. The user-space line width
                        // (`gs.line_width`) must therefore be divided by `s`
                        // here so the resulting device-space stroke width is
                        // the same as a regular path stroke at the same line
                        // width. Note: `s` is positive and bounded > 0
                        // because `font_size > 0` and `upm > 0`.
                        let local_width = state.current.line_width / s;
                        renderer.stroke_cached_path_with_width(
                            &path, &state.current, local_width,
                        );
                    }
                    // Synthetic bold for fonts whose FontDescriptor /Flags
                    // has bit 19 (ForceBold) set per ISO 32000-1 §9.8.1
                    // Table 122. We only synthesize when the user-requested
                    // render mode actually paints a fill (modes 0/2/4/6) and
                    // we're not already overlaying a stroke from mode 2 — in
                    // that case the explicit content-stream stroke is
                    // authoritative and the synthetic stroke would double up.
                    //
                    // The stroke uses the FILL color (so it appears as a
                    // bolder fill, not a separate outline) at ~3% of font
                    // size. After the per-glyph scale `s` the local-space
                    // width must be `font_size * 0.03 / s` to land at the
                    // correct device-space thickness.
                    if font_entry.force_bold && do_fill && !do_stroke {
                        let bold_local_width = (font_size * 0.03) / s;
                        // Temporarily mirror fill into stroke + clear dash so
                        // stroke_cached_path_with_width paints in the fill
                        // color with a solid line.
                        state.current.stroke_color = fill_rgba;
                        state.current.dash_array = Vec::new();
                        renderer.stroke_cached_path_with_width(
                            &path, &state.current, bold_local_width,
                        );
                    }
                    state.current.ctm = saved_ctm;
                    state.current.fill_color = saved_fill;
                    state.current.stroke_color = saved_stroke;
                    state.current.line_width = saved_line_width;
                    state.current.dash_array = saved_dash;
                }
            }

            // Width source: PDF /Widths array first, embedded font hmtx
            // as fallback (see pdf_advance_width comment).
            let w0 = pdf_advance_width(
                font_entry,
                byte as u32,
                outline.advance_width / upm,
            );
            let tw = if byte == 32 { word_spacing } else { 0.0 };
            let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
            tm[4] += tx * tm[0];
            tm[5] += tx * tm[1];
        }
    }
}

/// Build a tiny-skia `Path` for a single glyph from its outline commands.
/// Used by the per-render glyph path cache. Returns a guaranteed-non-empty
/// Path; callers that need to handle the empty-outline case should use
/// `build_glyph_path_opt`.
fn build_glyph_path(cmds: &[OutlineCommand]) -> tiny_skia::Path {
    build_glyph_path_opt(cmds).unwrap_or_else(|| {
        // The cache contract assumes the caller already filtered empty
        // outlines (commands.is_empty() check at the call site). If we
        // somehow get here, return a dummy single-point path so the
        // type signature stays infallible — it will never paint anything.
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(0.0, 0.0);
        pb.finish().expect("dummy 1-pt path")
    })
}

fn build_glyph_path_opt(cmds: &[OutlineCommand]) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    for cmd in cmds {
        match cmd {
            OutlineCommand::MoveTo(x, y) => pb.move_to(*x, *y),
            OutlineCommand::LineTo(x, y) => pb.line_to(*x, *y),
            OutlineCommand::CubicTo(x1, y1, x2, y2, x, y) => pb.cubic_to(*x1, *y1, *x2, *y2, *x, *y),
            OutlineCommand::Close => pb.close(),
        }
    }
    pb.finish()
}

/// Snap the per-glyph origin to the nearest device pixel.
///
/// PyMuPDF/MuPDF (the reference renderer) performs pixel-grid alignment
/// when rasterising glyph outlines: each glyph's origin is rounded to an
/// integer device pixel before the rasterizer scan-converts the outline.
/// This is a long-standing convention in font rasterizers (FreeType,
/// Cairo, Skia) — it produces crisper anti-aliasing edges and avoids the
/// 0.5-pixel "soft-edge" patterns that arise when a glyph stem straddles
/// two pixels at sub-pixel offset.
///
/// Without snapping, our renderer keeps the full sub-pixel position from
/// the text-matrix advance accumulator, so each glyph edge falls at a
/// fractional pixel. Both renderers produce visually correct text, but
/// per-pixel AA values differ across thousands of glyph edges → 4-8% diff
/// across text-heavy pages even though the layout is byte-identical.
///
/// We snap to whole device pixels (integer round) by:
///   1. Applying the current CTM to (gx, gy) to get device-space origin.
///   2. Rounding both to the nearest integer.
///   3. Applying the inverse CTM to recover the equivalent user-space gx'.
///
/// **Axis-aligned only**: snapping is only applied when the CTM has
/// negligible rotation/skew (kx/ky near zero). On rotated pages
/// (`/Rotate 90/270`) the page-level rotation is folded into the CTM
/// so kx/ky ≠ 0; in that regime, integer-device-pixel snapping shifts
/// glyphs along the perpendicular advance direction and can WORSEN the
/// AA pattern relative to the reference. PyMuPDF/MuPDF handle this
/// internally; we approximate the same behaviour by leaving rotated
/// text unsnapped (iter-7 found this regressed Technische tekening p0
/// and Barn Relocation p6, both `/Rotate 90` pages).
///
/// If the CTM is non-invertible (degenerate scale), we fall back to the
/// unsnapped origin.
#[inline]
fn snap_glyph_origin(gx: f32, gy: f32, ctm: &tiny_skia::Transform) -> (f32, f32) {
    // Skip snapping on rotated/skewed pages — the snap shifts each glyph
    // along the perpendicular advance direction by up to half a pixel,
    // and the reference renderer's AA pattern doesn't match that shift.
    const AXIS_ALIGNED_EPS: f32 = 1e-3;
    if ctm.kx.abs() > AXIS_ALIGNED_EPS || ctm.ky.abs() > AXIS_ALIGNED_EPS {
        return (gx, gy);
    }
    // Forward map (gx, gy) through CTM to device space.
    let dx = ctm.sx * gx + ctm.kx * gy + ctm.tx;
    let dy = ctm.ky * gx + ctm.sy * gy + ctm.ty;
    // Round to nearest pixel.
    let sdx = dx.round();
    let sdy = dy.round();
    // Inverse-map back to user space.
    match ctm.invert() {
        Some(inv) => {
            let sgx = inv.sx * sdx + inv.kx * sdy + inv.tx;
            let sgy = inv.ky * sdx + inv.sy * sdy + inv.ty;
            (sgx, sgy)
        }
        None => (gx, gy),
    }
}

/// Render CID-encoded (2-byte char codes) text directly into a SkiaRenderer.
///
/// Mirrors `render_cid_text_glyphs` but paints into the renderer instead of
/// emitting draw-command bytes. See `render_text_glyphs_skia` for the
/// design rationale.
pub fn render_cid_text_glyphs_skia(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: (u8, u8, u8, u8),
    renderer: &mut SkiaRenderer,
    state: &mut GraphicsStateStack,
    glyph_cache: Option<(lopdf::ObjectId, &mut GlyphPathCache)>,
) {
    render_cid_text_glyphs_skia_with_mode(
        text_bytes, font_entry, font_size, horizontal_scaling,
        char_spacing, word_spacing, rise, tm, fill_rgba,
        renderer, state, glyph_cache, 0,
    );
}

/// CID-text variant of `render_text_glyphs_skia_with_mode`. See that
/// function's docs for the meaning of `render_mode`.
pub fn render_cid_text_glyphs_skia_with_mode(
    text_bytes: &[u8],
    font_entry: &FontEntry,
    font_size: f32,
    horizontal_scaling: f32,
    char_spacing: f32,
    word_spacing: f32,
    rise: f32,
    tm: &mut [f32; 6],
    fill_rgba: (u8, u8, u8, u8),
    renderer: &mut SkiaRenderer,
    state: &mut GraphicsStateStack,
    glyph_cache: Option<(lopdf::ObjectId, &mut GlyphPathCache)>,
    render_mode: u8,
) {
    let parsed = match &font_entry.parsed {
        Some(p) => p,
        None => return,
    };

    let upm = parsed.units_per_em as f32;
    let (font_id_opt, mut cache_opt) = match glyph_cache {
        Some((id, cache)) => (Some(id), Some(cache)),
        None => (None, None),
    };

    let do_fill = matches!(render_mode, 0 | 2 | 4 | 6);
    let do_stroke = matches!(render_mode, 1 | 2 | 5 | 6);

    let mut i = 0;
    while i + 1 < text_bytes.len() {
        let cid = u16::from_be_bytes([text_bytes[i], text_bytes[i + 1]]);
        i += 2;

        let glyph_id = match FontRegistry::cid_to_glyph_id(font_entry, cid) {
            Some(id) => id,
            None => {
                let w0 = 0.5;
                let tx = (w0 * font_size + char_spacing) * horizontal_scaling;
                tm[4] += tx * tm[0];
                tm[5] += tx * tm[1];
                continue;
            }
        };

        if let Some(outline) = parsed.glyphs.get(&glyph_id) {
            if !outline.commands.is_empty() && (do_fill || do_stroke) {
                let s = font_size / upm;
                let sh = s * horizontal_scaling;
                let gx = rise * tm[2] + tm[4];
                let gy = rise * tm[3] + tm[5];
                let (sgx, sgy) = snap_glyph_origin(gx, gy, &state.current.ctm);

                let path_opt = if let (Some(font_id), Some(cache)) = (font_id_opt, cache_opt.as_deref_mut()) {
                    Some(cache.entry((font_id, glyph_id as u32))
                        .or_insert_with(|| build_glyph_path(&outline.commands))
                        .clone())
                } else {
                    build_glyph_path_opt(&outline.commands)
                };

                if let Some(path) = path_opt {
                    // See comment in render_text_glyphs_skia: avoid full
                    // state.save()/restore() to skip cloning the clip mask.
                    let saved_ctm = state.current.ctm;
                    let saved_fill = state.current.fill_color;
                    let saved_stroke = state.current.stroke_color;
                    let saved_line_width = state.current.line_width;
                    let saved_dash = state.current.dash_array.clone();
                    state.current.fill_color = fill_rgba;
                    state.current.ctm = state.current.ctm.pre_concat(
                        tiny_skia::Transform::from_row(
                            sh * tm[0], sh * tm[1], s * tm[2], s * tm[3], sgx, sgy,
                        ),
                    );
                    if do_fill {
                        renderer.fill_cached_path(&path, &state.current, false);
                    }
                    if do_stroke {
                        let local_width = state.current.line_width / s;
                        renderer.stroke_cached_path_with_width(
                            &path, &state.current, local_width,
                        );
                    }
                    // Synthetic bold for ForceBold flag — see the simple-text
                    // variant above for the rationale and width derivation.
                    if font_entry.force_bold && do_fill && !do_stroke {
                        let bold_local_width = (font_size * 0.03) / s;
                        state.current.stroke_color = fill_rgba;
                        state.current.dash_array = Vec::new();
                        renderer.stroke_cached_path_with_width(
                            &path, &state.current, bold_local_width,
                        );
                    }
                    state.current.ctm = saved_ctm;
                    state.current.fill_color = saved_fill;
                    state.current.stroke_color = saved_stroke;
                    state.current.line_width = saved_line_width;
                    state.current.dash_array = saved_dash;
                }
            }

            // Width source: PDF /W array first (keyed by CID), embedded
            // font hmtx as fallback (see pdf_advance_width comment).
            let w0 = pdf_advance_width(
                font_entry,
                cid as u32,
                outline.advance_width / upm,
            );
            let tw = if cid == 3 || cid == 32 { word_spacing } else { 0.0 };
            let tx = (w0 * font_size + char_spacing + tw) * horizontal_scaling;
            tm[4] += tx * tm[0];
            tm[5] += tx * tm[1];
        }
    }
}
