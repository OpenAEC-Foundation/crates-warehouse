//! Axis scaling helpers.

pub struct LinearAxis { pub min: f64, pub max: f64, pub px_start: f64, pub px_end: f64 }

impl LinearAxis {
    pub fn project(&self, value: f64) -> f64 {
        let range = self.max - self.min;
        if range.abs() < f64::EPSILON { return self.px_start; }
        let t = (value - self.min) / range;
        self.px_start + t * (self.px_end - self.px_start)
    }
}

pub fn nice_max(value: f64) -> f64 {
    if value <= 0.0 { return 1.0; }
    let pow = 10f64.powi(value.log10().floor() as i32);
    let n = (value / pow).ceil();
    let r = if n <= 1.0 { 1.0 } else if n <= 2.0 { 2.0 } else if n <= 5.0 { 5.0 } else { 10.0 };
    r * pow
}

/// Step ladder for the depth (NAP) axis — largest steps first.
///
/// Mirrors `DEPTH_LADDER` in `apps/desktop/src/components/chart/chart-renderer.ts`
/// so the in-app Home chart and the PDF report agree on tick positions
/// (e.g. labels at +0 / -5 / -10 for a ~30 m sondering instead of
/// per-metre noise). Sub-metre values are listed so that a future
/// zoomed report still has reasonable granularity.
pub const DEPTH_LADDER: &[f64] = &[10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05];

/// Pick the smallest step from `ladder` such that no more than
/// `max_ticks` ticks land in `range`. Returns the finest step if no
/// candidate fits (caller can clamp).
pub fn pick_ladder_step(range: f64, ladder: &[f64], max_ticks: usize) -> f64 {
    if range <= 0.0 { return ladder.first().copied().unwrap_or(1.0); }
    for &s in ladder {
        if range / s <= max_ticks as f64 {
            return s;
        }
    }
    *ladder.last().unwrap_or(&1.0)
}
