use tiny_skia::*;
use crate::graphics_state::GraphicsState;

pub struct SkiaRenderer {
    pub pixmap: Pixmap,
    path_builder: Option<PathBuilder>,
    width: u32,
    height: u32,
}

impl SkiaRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let mut pixmap = Pixmap::new(width, height)
            .ok_or_else(|| "Failed to create pixmap".to_string())?;
        pixmap.fill(Color::WHITE);
        Ok(SkiaRenderer { pixmap, path_builder: None, width, height })
    }

    /// Allocate an off-screen renderer of the SAME pixel dimensions as
    /// `self`, initialised to fully-transparent black. Used by the
    /// transparency-group code path: a Form XObject with `/Group /S
    /// /Transparency` must paint its contents into an isolated buffer that
    /// is then composited onto the parent at the parent's current /ca.
    /// Same-size guarantees that the inherited CTM and clip mask continue
    /// to address the same device-space pixel grid without remapping.
    pub fn new_offscreen_like(&self) -> Result<Self, String> {
        let pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| "Failed to create offscreen pixmap".to_string())?;
        // Pixmap::new initialises to all-zero (transparent black); leave it.
        Ok(SkiaRenderer {
            pixmap,
            path_builder: None,
            width: self.width,
            height: self.height,
        })
    }

    /// Composite an offscreen group buffer back onto `self` using
    /// PDF transparency-group semantics: full SourceOver blend at the
    /// captured group constant alpha. This is the painted-on-the-parent
    /// step described in PDF spec §11.6.6.
    pub fn composite_group(&mut self, sub: &SkiaRenderer, group_alpha: f32) {
        let paint = PixmapPaint {
            opacity: group_alpha.clamp(0.0, 1.0),
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        };
        // No clip is applied here on purpose: the inner draws have ALREADY
        // honoured the parent's clip mask (the same `gs.clip_path` was
        // shared via the GraphicsStateStack). Re-applying it here would
        // double-clip and produce nothing different at best, garbage at
        // worst — leave it alone.
        self.pixmap.draw_pixmap(
            0, 0,
            sub.pixmap.as_ref(),
            &paint,
            Transform::identity(),
            None,
        );
    }

    /// Snapshot the current path-builder contents into a finished Path
    /// without consuming the builder. Used by the clipping operator (`W` /
    /// `W*`), which needs to apply the same path to the GraphicsState clip
    /// mask in addition to whatever the immediately-following paint
    /// operator does with it. Returns `None` if the path is empty.
    pub fn snapshot_path(&self) -> Option<Path> {
        let pb = self.path_builder.as_ref()?;
        pb.clone().finish()
    }

    /// PDF `W` / `W*` clipping. The supplied path (already in user space)
    /// is intersected with the existing clip mask in `gs.clip_path`,
    /// mapping through `gs.ctm` to pixmap pixel coordinates. If no clip
    /// mask exists yet, a new one is created from the path; subsequent
    /// `W` operators inside nested `q`/`Q` blocks intersect with it.
    /// `q` clones the GraphicsState (including the Mask), so `Q`
    /// automatically restores the previous clip — that's how PDF
    /// clip-stack semantics work without any extra plumbing.
    ///
    /// Iter 33: when the clip path is an axis-aligned rectangle and the
    /// CTM is scale+translate (no rotation/skew), the device-space rect's
    /// edges may fall in the lower half of a pixel cell. With AA fill
    /// the boundary row gets only partial coverage (~57% in the Tekst
    /// p2/p3 case), attenuating any content drawn through that clip.
    /// MuPDF/PyMuPDF instead use any-pixel-touched semantics for the
    /// clip rect, so the boundary row gets full coverage. Mirror that
    /// here by inflating the device-space rect outward by 0.5 px on each
    /// edge before building the mask. Non-rect or non-orthogonal clips
    /// fall back to the original behaviour (the residual issue is
    /// dominated by the simple `re W n` axis-aligned rect case).
    pub fn apply_clip(&mut self, gs: &mut GraphicsState, path: &Path, even_odd: bool) {
        let fill_rule = if even_odd { FillRule::EvenOdd } else { FillRule::Winding };

        // Try the axis-aligned-rect fast path. When it fires, we transform
        // the rect into device space, inflate by 0.5 px on each edge, and
        // fill a fresh rect path with identity transform. Otherwise we fall
        // back to the original CTM-transformed fill.
        if let Some(inflated_path) = Self::inflate_axis_aligned_rect_clip(path, &gs.ctm) {
            match gs.clip_path.as_mut() {
                Some(mask) => {
                    mask.intersect_path(&inflated_path, fill_rule, true, Transform::identity());
                }
                None => {
                    if let Some(mut mask) = Mask::new(self.width, self.height) {
                        mask.fill_path(&inflated_path, fill_rule, true, Transform::identity());
                        gs.clip_path = Some(mask);
                    }
                }
            }
            return;
        }

        match gs.clip_path.as_mut() {
            Some(mask) => {
                mask.intersect_path(path, fill_rule, true, gs.ctm);
            }
            None => {
                if let Some(mut mask) = Mask::new(self.width, self.height) {
                    mask.fill_path(path, fill_rule, true, gs.ctm);
                    gs.clip_path = Some(mask);
                }
            }
        }
    }

    /// If `path` is an axis-aligned rectangle (4 line segments meeting at
    /// right angles, possibly with an explicit Close) AND `ctm` is a pure
    /// scale+translate, return a NEW path containing the device-space rect
    /// inflated outward by 0.5 px on each edge. Returns `None` for any
    /// other shape so the caller can fall back to the original code path.
    fn inflate_axis_aligned_rect_clip(path: &Path, ctm: &Transform) -> Option<Path> {
        // Only orthogonal CTMs preserve axis-alignment; a rotated CTM
        // would turn the rect into a parallelogram and the bbox-inflate
        // trick would over-clip the corners.
        if !ctm.is_scale_translate() {
            return None;
        }

        // Walk the segments and verify it's the canonical 4-line rect
        // (M, L, L, L, optional Close) with all edges strictly horizontal
        // or vertical in user space.
        let mut iter = path.segments();
        let p0 = match iter.next()? {
            PathSegment::MoveTo(p) => p,
            _ => return None,
        };
        let mut pts = [p0; 4];
        for slot in pts.iter_mut().skip(1) {
            match iter.next()? {
                PathSegment::LineTo(p) => *slot = p,
                _ => return None,
            }
        }
        // Optional Close, optional return-to-start LineTo, then end.
        loop {
            match iter.next() {
                Some(PathSegment::Close) => continue,
                Some(PathSegment::LineTo(p)) if (p.x - p0.x).abs() < 1e-4
                    && (p.y - p0.y).abs() < 1e-4 =>
                {
                    continue;
                }
                None => break,
                _ => return None,
            }
        }
        // Verify axis-aligned: edges 0->1, 1->2, 2->3, 3->0 each strictly
        // horizontal or vertical, alternating.
        let edges = [
            (pts[0], pts[1]),
            (pts[1], pts[2]),
            (pts[2], pts[3]),
            (pts[3], pts[0]),
        ];
        for (a, b) in edges.iter() {
            let horiz = (a.y - b.y).abs() < 1e-4 && (a.x - b.x).abs() > 1e-4;
            let vert  = (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() > 1e-4;
            if !horiz && !vert {
                return None;
            }
        }

        // Compute the device-space bbox by mapping all 4 corners through
        // the (orthogonal) CTM.
        let mut corners = pts;
        ctm.map_points(&mut corners);
        let (mut minx, mut miny) = (corners[0].x, corners[0].y);
        let (mut maxx, mut maxy) = (corners[0].x, corners[0].y);
        for p in &corners[1..] {
            if p.x < minx { minx = p.x; } else if p.x > maxx { maxx = p.x; }
            if p.y < miny { miny = p.y; } else if p.y > maxy { maxy = p.y; }
        }

        // Outward-round by 0.5 device px on each edge — see method-level
        // comment for the rationale (iter 33).
        const PAD: f32 = 0.5;
        let rect = Rect::from_ltrb(minx - PAD, miny - PAD, maxx + PAD, maxy + PAD)?;
        let path = PathBuilder::from_rect(rect);
        Some(path)
    }

    pub fn begin_path(&mut self) {
        self.path_builder = Some(PathBuilder::new());
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        if let Some(ref mut pb) = self.path_builder { pb.move_to(x, y); }
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        if let Some(ref mut pb) = self.path_builder { pb.line_to(x, y); }
    }

    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
        if let Some(ref mut pb) = self.path_builder { pb.cubic_to(x1, y1, x2, y2, x3, y3); }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(ref mut pb) = self.path_builder {
            pb.move_to(x, y);
            pb.line_to(x + w, y);
            pb.line_to(x + w, y + h);
            pb.line_to(x, y + h);
            pb.close();
        }
    }

    pub fn close_path(&mut self) {
        if let Some(ref mut pb) = self.path_builder { pb.close(); }
    }

    /// Multiply ExtGState constant alpha (`/ca` or `/CA`) into the per-color
    /// alpha byte. Both factors are clamped to `[0, 1]` first.
    fn blend_alpha(color_a: u8, gs_alpha: f32) -> u8 {
        let alpha = (color_a as f32 / 255.0) * gs_alpha.clamp(0.0, 1.0);
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    pub fn fill(&mut self, gs: &GraphicsState, even_odd: bool) {
        let path = match self.path_builder.take() {
            Some(pb) => match pb.finish() { Some(p) => p, None => return },
            None => return,
        };
        let mut paint = Paint::default();
        let (r, g, b, a) = gs.fill_color;
        paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_fill_alpha()));
        paint.anti_alias = true;
        let rule = if even_odd { FillRule::EvenOdd } else { FillRule::Winding };
        self.pixmap.fill_path(&path, &paint, rule, gs.ctm, gs.clip_path.as_ref());
    }

    /// Fill a pre-built path. Used by the per-render glyph path cache
    /// (`text_renderer::render_*_glyphs_skia`) to avoid re-tessellating
    /// the same glyph outline for every instance on a page. Behaviour
    /// matches `fill` except the path is supplied directly rather than
    /// taken from the in-progress `path_builder`.
    pub fn fill_cached_path(&mut self, path: &Path, gs: &GraphicsState, even_odd: bool) {
        let mut paint = Paint::default();
        let (r, g, b, a) = gs.fill_color;
        paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_fill_alpha()));
        paint.anti_alias = true;
        let rule = if even_odd { FillRule::EvenOdd } else { FillRule::Winding };
        self.pixmap.fill_path(path, &paint, rule, gs.ctm, gs.clip_path.as_ref());
    }

    /// Stroke a pre-built glyph path with an explicit path-local stroke
    /// width. Used by the text-renderer to honour PDF text rendering modes
    /// 1, 2, 5 and 6 (PDF 1.7 §9.3.6 Table 106). The caller is responsible
    /// for converting the user-space line width (`gs.line_width`) to the
    /// path-local width by dividing by the per-glyph font scale `s`, so
    /// that after the CTM (which already includes `s` via pre_concat) the
    /// device-space stroke width equals `gs.line_width * page_scale` —
    /// matching the way path strokes outside text behave.
    ///
    /// The stroke colour comes from `gs.stroke_color` (NOT `fill_color`),
    /// per PDF spec: text-rendering mode 2 fills with the non-stroking
    /// colour space and strokes with the stroking colour space. Common
    /// authoring tools set both to the same value when emulating bold,
    /// but the spec is explicit that they are independent.
    pub fn stroke_cached_path_with_width(
        &mut self,
        path: &Path,
        gs: &GraphicsState,
        path_local_width: f32,
    ) {
        if path_local_width <= 0.0 {
            return;
        }
        let mut paint = Paint::default();
        let (r, g, b, a) = gs.stroke_color;
        paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_stroke_alpha()));
        paint.anti_alias = true;

        let mut stroke = Stroke::default();
        stroke.width = path_local_width;
        stroke.line_cap = match gs.line_cap {
            1 => LineCap::Round,
            2 => LineCap::Square,
            _ => LineCap::Butt,
        };
        stroke.line_join = match gs.line_join {
            1 => LineJoin::Round,
            2 => LineJoin::Bevel,
            _ => LineJoin::Miter,
        };
        stroke.miter_limit = gs.miter_limit;
        if !gs.dash_array.is_empty() {
            stroke.dash = StrokeDash::new(gs.dash_array.clone(), gs.dash_phase);
        }
        self.pixmap
            .stroke_path(path, &paint, &stroke, gs.ctm, gs.clip_path.as_ref());
    }

    /// Resolve the user-space stroke width applied to tiny_skia.
    ///
    /// PDF spec section 8.4.3.2: `w 0` (line width 0) means "thinnest line
    /// that can be rendered at device resolution: 1 device pixel". The spec
    /// notes that on high-resolution devices, such lines are nearly
    /// invisible. PyMuPDF/MuPDF render them as faint sub-pixel hairlines,
    /// and tiny_skia's default behaviour for `width=0` produces a full
    /// coverage 1px hairline — which is 2-3× heavier than MuPDF's output
    /// for engineering drawings (e.g. AutoCAD-exported PDFs that use `0 w`
    /// throughout).
    ///
    /// To match the reference, we substitute a tiny positive width such
    /// that, after the current CTM is applied, the device-space width is
    /// approximately 0.2 pixels — a quarter hairline. tiny_skia's
    /// `treat_as_hairline` (in `painter.rs`) then returns ~0.2 coverage,
    /// producing a low-opacity 1px hairline that visually matches
    /// MuPDF/PyMuPDF's rendering of zero-width lines on engineering drawings.
    /// The 0.2 value was tuned against the test suite — pages with many
    /// `w 0` lines (Technische tekening A1 floor plans) match within 2-3%
    /// of the reference at this setting; bigger values leave the lines too
    /// dark, smaller values lose stroke detail.
    fn resolve_stroke_width(gs: &GraphicsState) -> f32 {
        if gs.line_width > 0.0 {
            return gs.line_width;
        }
        // Estimate the dominant CTM scale (geometric mean of column lengths).
        // This handles rotation+scale CTMs without bias.
        let t = gs.ctm;
        let sx = (t.sx * t.sx + t.kx * t.kx).sqrt();
        let sy = (t.ky * t.ky + t.sy * t.sy).sqrt();
        let scale = (sx * sy).sqrt();
        if scale > 0.0 {
            // Aim for ~0.2 device-pixel device-space width, so tiny_skia's
            // hairline coverage modulation kicks in at ~20% opacity. This
            // matches the apparent stroke density in PyMuPDF/MuPDF reference
            // renders for engineering drawings (AutoCAD-exported PDFs).
            0.2 / scale
        } else {
            0.0
        }
    }

    pub fn stroke(&mut self, gs: &GraphicsState) {
        let path = match self.path_builder.take() {
            Some(pb) => match pb.finish() { Some(p) => p, None => return },
            None => return,
        };
        let mut paint = Paint::default();
        let (r, g, b, a) = gs.stroke_color;
        paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_stroke_alpha()));
        paint.anti_alias = true;

        let mut stroke = Stroke::default();
        stroke.width = Self::resolve_stroke_width(gs);
        stroke.line_cap = match gs.line_cap { 1 => LineCap::Round, 2 => LineCap::Square, _ => LineCap::Butt };
        stroke.line_join = match gs.line_join { 1 => LineJoin::Round, 2 => LineJoin::Bevel, _ => LineJoin::Miter };
        stroke.miter_limit = gs.miter_limit;
        if !gs.dash_array.is_empty() {
            stroke.dash = StrokeDash::new(gs.dash_array.clone(), gs.dash_phase);
        }
        self.pixmap.stroke_path(&path, &paint, &stroke, gs.ctm, gs.clip_path.as_ref());
    }

    pub fn fill_and_stroke(&mut self, gs: &GraphicsState, even_odd: bool) {
        if let Some(pb) = self.path_builder.take() {
            if let Some(path) = pb.finish() {
                // Fill
                let mut fill_paint = Paint::default();
                let (r, g, b, a) = gs.fill_color;
                fill_paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_fill_alpha()));
                fill_paint.anti_alias = true;
                let rule = if even_odd { FillRule::EvenOdd } else { FillRule::Winding };
                self.pixmap.fill_path(&path, &fill_paint, rule, gs.ctm, gs.clip_path.as_ref());
                // Stroke
                let mut stroke_paint = Paint::default();
                let (r, g, b, a) = gs.stroke_color;
                stroke_paint.set_color_rgba8(r, g, b, Self::blend_alpha(a, gs.effective_stroke_alpha()));
                stroke_paint.anti_alias = true;
                let mut stroke = Stroke::default();
                stroke.width = Self::resolve_stroke_width(gs);
                stroke.line_cap = match gs.line_cap { 1 => LineCap::Round, 2 => LineCap::Square, _ => LineCap::Butt };
                stroke.line_join = match gs.line_join { 1 => LineJoin::Round, 2 => LineJoin::Bevel, _ => LineJoin::Miter };
                stroke.miter_limit = gs.miter_limit;
                if !gs.dash_array.is_empty() {
                    stroke.dash = StrokeDash::new(gs.dash_array.clone(), gs.dash_phase);
                }
                self.pixmap.stroke_path(&path, &stroke_paint, &stroke, gs.ctm, gs.clip_path.as_ref());
            }
        }
    }

    pub fn draw_image(&mut self, width: u32, height: u32, rgba_pixels: &[u8], gs: &GraphicsState) {
        let img = match PixmapRef::from_bytes(rgba_pixels, width, height) {
            Some(p) => p,
            None => return,
        };
        if width == 0 || height == 0 { return; }
        // PDF Image XObjects live in a 1×1 unit square; the caller's CTM is
        // set up so that unit square maps to the destination region. But
        // `draw_pixmap` consumes a transform that maps the SOURCE PIXMAP
        // PIXEL SPACE (0..width × 0..height) to the destination. So we
        // need to pre-scale by 1/width and 1/height to convert pixel
        // coordinates into the unit square before the caller's CTM
        // applies its unit-square → destination mapping.
        let pixel_to_unit = Transform::from_scale(1.0 / width as f32, 1.0 / height as f32);
        let final_xform = gs.ctm.pre_concat(pixel_to_unit);
        // Iter 32: tiny_skia's `draw_pixmap` fills the destination rect with
        // pixel-center-inside semantics (non-AA), so a destination edge that
        // falls at e.g. pixel y=2822.43 LEAVES pixel row 2822 (centered at
        // 2822.5) UNFILLED — giving Tekst.pdf p2 a single missing footer
        // row vs. PyMuPDF reference. PyMuPDF (and MuPDF underneath) instead
        // rasterises the image rect with OUTWARD rounding so any pixel even
        // partially touched is covered by a bilinear sample; with
        // SpreadMode::Pad on the pattern, edge pixels just replicate the
        // closest source row. Replicate that behaviour here by manually
        // constructing the Pattern shader and filling a path that we expand
        // by a small amount in source space — enough to push the destination
        // rect just past the next pixel boundary on each side.
        let pad = 0.5_f32; // source-pixel padding; rounds dest rect outward
        let expanded_src = Rect::from_ltrb(
            -pad,
            -pad,
            width as f32 + pad,
            height as f32 + pad,
        );
        if let Some(rect) = expanded_src {
            let path = PathBuilder::from_rect(rect);
            let pattern = Pattern::new(
                img,
                SpreadMode::Pad,
                FilterQuality::Bilinear,
                gs.effective_fill_alpha(),
                Transform::identity(),
            );
            let paint = Paint {
                shader: pattern,
                blend_mode: BlendMode::SourceOver,
                anti_alias: false,
                force_hq_pipeline: false,
            };
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                final_xform,
                gs.clip_path.as_ref(),
            );
        } else {
            // Fallback to the original code path if rect construction fails.
            let paint = PixmapPaint {
                opacity: gs.effective_fill_alpha(),
                blend_mode: BlendMode::SourceOver,
                quality: FilterQuality::Bilinear,
            };
            self.pixmap.draw_pixmap(0, 0, img, &paint, final_xform, gs.clip_path.as_ref());
        }
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.pixmap.data().to_vec()
    }
}
