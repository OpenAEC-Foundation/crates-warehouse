pub fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> (u8, u8, u8) {
    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);
    (r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8)
}

pub fn gray_to_rgb(g: f32) -> (u8, u8, u8) {
    let v = (g * 255.0).clamp(0.0, 255.0) as u8;
    (v, v, v)
}

pub fn rgb_to_rgba8(r: f32, g: f32, b: f32) -> (u8, u8, u8, u8) {
    (
        (r * 255.0).clamp(0.0, 255.0) as u8,
        (g * 255.0).clamp(0.0, 255.0) as u8,
        (b * 255.0).clamp(0.0, 255.0) as u8,
        255,
    )
}
