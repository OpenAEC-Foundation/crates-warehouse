//! Bounding-box accumulator.
//!
//! Tessellators call [`expand`] for every emitted point so the walker
//! can hand the final `[xmin, ymin, xmax, ymax]` to `DxfSink::finalize`.

/// Grow `bbox` (`[xmin, ymin, xmax, ymax]`) to include `(x, y)`.
///
/// Non-finite values are ignored; absurdly large values (`|x| > 1e6`
/// or `|y| > 1e6`) are clipped so a single garbage-decoded coordinate
/// doesn't collapse a sensible camera frame to a single pixel.
#[inline]
pub fn expand(bbox: &mut [f64; 4], x: f64, y: f64) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    if x.abs() > 1.0e6 || y.abs() > 1.0e6 {
        return;
    }
    if x < bbox[0] {
        bbox[0] = x;
    }
    if y < bbox[1] {
        bbox[1] = y;
    }
    if x > bbox[2] {
        bbox[2] = x;
    }
    if y > bbox[3] {
        bbox[3] = y;
    }
}

/// Initial sentinel value — `xmin = ymin = +inf`, `xmax = ymax = -inf`.
pub const INIT: [f64; 4] = [
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
];
