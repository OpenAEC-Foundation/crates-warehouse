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
