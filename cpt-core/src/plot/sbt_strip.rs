//! Vertical Robertson SBT colour strip.

use crate::layers::detect_layers;
use crate::domain::Cpt;
use super::axes::LinearAxis;

pub fn render(cpt: &Cpt, y_axis: &LinearAxis, x: f64, width: f64) -> String {
    let mut out = String::new();
    for layer in detect_layers(cpt) {
        let y_top = y_axis.project(layer.depth_top);
        let y_bot = y_axis.project(layer.depth_bottom);
        out.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" />"#,
            x, y_top, width, (y_bot - y_top).max(0.0), layer.zone_color
        ));
    }
    out
}
