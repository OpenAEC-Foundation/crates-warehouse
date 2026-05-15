//! Curve path generation for qc, fs, Rf.

use crate::domain::Cpt;
use super::axes::LinearAxis;

pub fn polyline_points<F>(cpt: &Cpt, x_axis: &LinearAxis, y_axis: &LinearAxis, value: F) -> String
where F: Fn(&crate::domain::MeasurementPoint) -> Option<f64>
{
    let mut s = String::new();
    for p in &cpt.points {
        if let Some(v) = value(p) {
            let x = x_axis.project(v);
            let y = y_axis.project(p.depth);
            if !s.is_empty() { s.push(' '); }
            s.push_str(&format!("{:.2},{:.2}", x, y));
        }
    }
    s
}
