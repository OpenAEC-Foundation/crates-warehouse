/// Compact binary format for Canvas2D draw commands.
/// Each command is a type byte (u8) followed by parameters in little-endian.

pub struct DrawCommandBuffer {
    data: Vec<u8>,
}

impl DrawCommandBuffer {
    pub fn new() -> Self {
        DrawCommandBuffer { data: Vec::new() }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        DrawCommandBuffer { data }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    // Command 0: MoveTo(x, y) — 1+8 bytes
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.data.push(0);
        self.push_f32(x);
        self.push_f32(y);
    }

    // Command 1: LineTo(x, y) — 1+8 bytes
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.data.push(1);
        self.push_f32(x);
        self.push_f32(y);
    }

    // Command 2: CubicTo(x1, y1, x2, y2, x3, y3) — 1+24 bytes
    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
        self.data.push(2);
        self.push_f32(x1);
        self.push_f32(y1);
        self.push_f32(x2);
        self.push_f32(y2);
        self.push_f32(x3);
        self.push_f32(y3);
    }

    // Command 3: Rect(x, y, w, h) — 1+16 bytes
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.data.push(3);
        self.push_f32(x);
        self.push_f32(y);
        self.push_f32(w);
        self.push_f32(h);
    }

    // Command 4: ClosePath — 1 byte
    pub fn close_path(&mut self) {
        self.data.push(4);
    }

    // Command 5: SetStroke(rgba u32, width f32) — 1+8 bytes
    pub fn set_stroke(&mut self, rgba: u32, width: f32) {
        self.data.push(5);
        self.push_u32(rgba);
        self.push_f32(width);
    }

    // Command 6: SetFill(rgba u32) — 1+4 bytes
    pub fn set_fill(&mut self, rgba: u32) {
        self.data.push(6);
        self.push_u32(rgba);
    }

    // Command 7: Stroke — 1 byte
    pub fn stroke(&mut self) {
        self.data.push(7);
    }

    // Command 8: Fill — 1 byte
    pub fn fill(&mut self) {
        self.data.push(8);
    }

    // Command 9: FillEvenOdd — 1 byte
    pub fn fill_even_odd(&mut self) {
        self.data.push(9);
    }

    // Command 10: SaveState — 1 byte
    pub fn save_state(&mut self) {
        self.data.push(10);
    }

    // Command 11: RestoreState — 1 byte
    pub fn restore_state(&mut self) {
        self.data.push(11);
    }

    // Command 12: Transform(a, b, c, d, e, f) — 1+24 bytes
    pub fn transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        self.data.push(12);
        self.push_f32(a);
        self.push_f32(b);
        self.push_f32(c);
        self.push_f32(d);
        self.push_f32(e);
        self.push_f32(f);
    }

    // Command 13: SetLineCap(u8) — 1+1 bytes
    pub fn set_line_cap(&mut self, cap: u8) {
        self.data.push(13);
        self.data.push(cap);
    }

    // Command 14: SetLineJoin(u8) — 1+1 bytes
    pub fn set_line_join(&mut self, join: u8) {
        self.data.push(14);
        self.data.push(join);
    }

    // Command 15: SetMiterLimit(f32) — 1+4 bytes
    pub fn set_miter_limit(&mut self, limit: f32) {
        self.data.push(15);
        self.push_f32(limit);
    }

    // Command 16: SetDash(count u8, pattern f32[], phase f32) — variable
    pub fn set_dash(&mut self, pattern: &[f32], phase: f32) {
        self.data.push(16);
        self.data.push(pattern.len() as u8);
        for &v in pattern {
            self.push_f32(v);
        }
        self.push_f32(phase);
    }

    // Command 17: BeginPath — 1 byte
    pub fn begin_path(&mut self) {
        self.data.push(17);
    }

    // Command 18: TextAt(x, y, fontSize, text) — variable length
    // u8 opcode + f32 x + f32 y + f32 fontSize + u32 rgba + u8 textLength + UTF-8 bytes
    pub fn text_at(&mut self, x: f32, y: f32, font_size: f32, rgba: u32, text: &str) {
        self.data.push(18);
        self.push_f32(x);
        self.push_f32(y);
        self.push_f32(font_size);
        self.push_u32(rgba);
        let bytes = text.as_bytes();
        let len = bytes.len().min(255);
        self.data.push(len as u8);
        self.data.extend_from_slice(&bytes[..len]);
    }

    // Command 20: Clip — apply current path as clipping region (nonzero winding)
    pub fn clip(&mut self) {
        self.data.push(20);
    }

    // Command 21: ClipEvenOdd — apply current path as clipping region (even-odd)
    pub fn clip_even_odd(&mut self) {
        self.data.push(21);
    }

    // Command 19: DrawImage(w, h, dataLen, imageBytes)
    // Draws an image at the current transform position.
    // The image is placed in a 1×1 unit square (PDF image space), so the caller
    // must set up a transform that maps this to the correct page position/size.
    // Format: u8 opcode + u16 width + u16 height + u32 dataLen + raw image bytes
    // Image data is encoded as JPEG/PNG (passthrough from PDF) or raw RGB/RGBA.
    pub fn draw_image(&mut self, width: u16, height: u16, image_bytes: &[u8]) {
        self.data.push(19);
        self.data.extend_from_slice(&width.to_le_bytes());
        self.data.extend_from_slice(&height.to_le_bytes());
        self.push_u32(image_bytes.len() as u32);
        self.data.extend_from_slice(image_bytes);
    }

    fn push_f32(&mut self, v: f32) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    fn push_u32(&mut self, v: u32) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_commands() {
        let mut buf = DrawCommandBuffer::new();
        buf.move_to(1.0, 2.0);
        buf.line_to(3.0, 4.0);
        buf.close_path();
        buf.stroke();

        let bytes = buf.into_bytes();
        assert_eq!(bytes[0], 0); // MoveTo
        assert_eq!(bytes[9], 1); // LineTo
        assert_eq!(bytes[18], 4); // ClosePath
        assert_eq!(bytes[19], 7); // Stroke
        assert_eq!(bytes.len(), 20);
    }

    #[test]
    fn test_from_vec_roundtrip() {
        let mut buf = DrawCommandBuffer::new();
        buf.begin_path();
        buf.rect(0.0, 0.0, 100.0, 50.0);
        buf.set_fill(0xFF0000FF);
        buf.fill();

        let bytes = buf.into_bytes();
        let restored = DrawCommandBuffer::from_vec(bytes.clone());
        assert_eq!(restored.into_bytes(), bytes);
    }

    #[test]
    fn test_set_dash_variable_length() {
        let mut buf = DrawCommandBuffer::new();
        buf.set_dash(&[5.0, 3.0, 1.0], 0.0);
        let bytes = buf.into_bytes();
        assert_eq!(bytes[0], 16); // SetDash
        assert_eq!(bytes[1], 3);  // count
        // 3 f32 pattern + 1 f32 phase = 16 bytes after count
        assert_eq!(bytes.len(), 1 + 1 + 12 + 4); // 18
    }
}
