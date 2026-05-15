//! SVG plot rendering for a single CPT, NEN-EN-ISO 22476-1 layout.
//!
//! Output is a self-contained SVG string that the openaec PDF engine
//! embeds via `resvg` (vector → raster at PDF resolution).

pub mod axes;
pub mod curves;
pub mod sbt_strip;

use crate::domain::Cpt;
use axes::{LinearAxis, nice_max};

const W: f64 = 600.0;
const H: f64 = 800.0;
const M_LEFT: f64 = 60.0;
const M_RIGHT: f64 = 30.0;
const M_TOP: f64 = 40.0;
const M_BOTTOM: f64 = 40.0;
const SBT_W: f64 = 18.0;
const SBT_GAP: f64 = 6.0;

pub fn render_cpt_svg(cpt: &Cpt) -> String {
    if cpt.points.is_empty() {
        return format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}">
<text x="{}" y="{}" text-anchor="middle" font-family="Inter" font-size="14" fill="#888">No data</text>
</svg>"##, W / 2.0, H / 2.0);
    }

    // Depth range (m below ground level)
    let max_depth = cpt.points.iter().map(|p| p.depth).fold(0.0_f64, f64::max);
    let y_axis = LinearAxis { min: 0.0, max: max_depth, px_start: M_TOP, px_end: H - M_BOTTOM };

    // qc range — auto-fit, nice round
    let max_qc = cpt.points.iter().filter_map(|p| p.qc).fold(0.0_f64, f64::max);
    let qc_max = nice_max(max_qc.max(1.0));
    let qc_axis = LinearAxis { min: 0.0, max: qc_max, px_start: M_LEFT, px_end: W - M_RIGHT - SBT_W - SBT_GAP };

    // Rf on a secondary scale 0..10% (reversed so larger Rf is at left)
    let rf_axis = LinearAxis { min: 10.0, max: 0.0, px_start: M_LEFT, px_end: W - M_RIGHT - SBT_W - SBT_GAP };

    // Curves
    let qc_points = curves::polyline_points(cpt, &qc_axis, &y_axis, |p| p.qc);
    let rf_points = curves::polyline_points(cpt, &rf_axis, &y_axis, |p| p.rf);

    // SBT strip on the right
    let sbt = sbt_strip::render(cpt, &y_axis, W - M_RIGHT - SBT_W, SBT_W);

    // Depth ticks every 1m
    let mut ticks = String::new();
    let mut d = 0.0;
    while d <= max_depth {
        let y = y_axis.project(d);
        ticks.push_str(&format!(
            r##"<line x1="{}" y1="{:.2}" x2="{}" y2="{:.2}" stroke="#E7E5E4" stroke-width="0.5" />
<text x="{}" y="{:.2}" font-family="JetBrains Mono" font-size="9" fill="#57534E" text-anchor="end" dominant-baseline="central">{:.1}</text>"##,
            M_LEFT, y, W - M_RIGHT - SBT_W - SBT_GAP, y,
            M_LEFT - 4.0, y, d
        ));
        d += 1.0;
    }

    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="Inter">
<rect x="0" y="0" width="{W}" height="{H}" fill="#FAFAF9" />
{ticks}
<polyline points="{qc_points}" fill="none" stroke="#D97706" stroke-width="1.2" />
<polyline points="{rf_points}" fill="none" stroke="#F59E0B" stroke-width="1.2" />
{sbt}
<text x="{}" y="20" font-family="Space Grotesk" font-weight="700" font-size="12" fill="#36363E">Sondering {}</text>
</svg>"##, M_LEFT, cpt.id)
}
