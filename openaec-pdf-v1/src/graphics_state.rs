use tiny_skia::{Mask, Transform};

#[derive(Clone, Debug)]
pub struct GraphicsState {
    pub ctm: Transform,
    pub fill_color: (u8, u8, u8, u8),
    pub stroke_color: (u8, u8, u8, u8),
    pub line_width: f32,
    pub line_cap: u8,
    pub line_join: u8,
    pub miter_limit: f32,
    pub dash_array: Vec<f32>,
    pub dash_phase: f32,
    /// Accumulated clip mask in pixmap (device) coordinates. PDF `W` / `W*`
    /// stores the path here as an alpha mask; subsequent paint operators
    /// pass it to `tiny_skia::Pixmap::{fill_path,stroke_path,draw_pixmap}`
    /// as the `mask` argument so painting respects the clip. Cloned by
    /// `q`, restored by `Q`.
    pub clip_path: Option<Mask>,
    /// Constant alpha for non-stroking operations (PDF /ca, ExtGState).
    /// Multiplied into fill_color alpha and image paint opacity. 0..1.
    pub fill_alpha: f32,
    /// Constant alpha for stroking operations (PDF /CA, ExtGState). 0..1.
    pub stroke_alpha: f32,
    /// Inherited group alpha from the enclosing transparency group(s).
    /// PDF Form XObjects with `/Group /S /Transparency` should be rendered
    /// into a separate buffer at full alpha and then composited onto the
    /// parent at the parent's current alpha. Since this renderer flattens
    /// everything into a single pixmap, we approximate by capturing the
    /// parent's `fill_alpha` when entering a transparency group, resetting
    /// `fill_alpha` to 1.0 inside the group, and multiplying both at draw
    /// time. (Same idea for the stroke side.)
    pub group_fill_alpha: f32,
    pub group_stroke_alpha: f32,
    /// Text rendering mode (PDF 1.7 §9.3.6 Table 106) — set by `Tr` operator.
    /// Per PDF 1.7 §8.4.1 the text state is part of the graphics state and
    /// therefore saved/restored by `q`/`Q`. Default 0 = fill only.
    /// Values: 0=fill, 1=stroke, 2=fill+stroke (synthetic-bold idiom),
    /// 3=invisible, 4-7 = same as 0-3 plus add to clipping path.
    pub text_render_mode: u8,
}

impl GraphicsState {
    /// Effective non-stroking alpha at draw time: group × current.
    pub fn effective_fill_alpha(&self) -> f32 {
        (self.group_fill_alpha * self.fill_alpha).clamp(0.0, 1.0)
    }
    /// Effective stroking alpha at draw time: group × current.
    pub fn effective_stroke_alpha(&self) -> f32 {
        (self.group_stroke_alpha * self.stroke_alpha).clamp(0.0, 1.0)
    }
}

impl Default for GraphicsState {
    fn default() -> Self {
        GraphicsState {
            ctm: Transform::identity(),
            fill_color: (0, 0, 0, 255),
            stroke_color: (0, 0, 0, 255),
            line_width: 1.0,
            line_cap: 0,
            line_join: 0,
            miter_limit: 10.0,
            dash_array: Vec::new(),
            dash_phase: 0.0,
            clip_path: None,
            fill_alpha: 1.0,
            stroke_alpha: 1.0,
            group_fill_alpha: 1.0,
            group_stroke_alpha: 1.0,
            text_render_mode: 0,
        }
    }
}

pub struct GraphicsStateStack {
    stack: Vec<GraphicsState>,
    pub current: GraphicsState,
}

impl GraphicsStateStack {
    pub fn new() -> Self {
        GraphicsStateStack {
            stack: Vec::new(),
            current: GraphicsState::default(),
        }
    }

    pub fn save(&mut self) {
        self.stack.push(self.current.clone());
    }

    pub fn restore(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.current = state;
        }
    }

    pub fn concat_matrix(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let new_transform = Transform::from_row(a, b, c, d, e, f);
        self.current.ctm = self.current.ctm.pre_concat(new_transform);
    }
}
