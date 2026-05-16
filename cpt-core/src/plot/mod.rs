//! SVG plot rendering for a single CPT — Dutch-CPT reference layout.
//!
//! Output is a self-contained A4-portrait SVG string. A companion
//! `render_cpt_png` rasterises the SVG with `resvg` so the openaec engine
//! (which only knows about raster images) can embed it directly.

pub mod axes;
pub mod curves;
pub mod sbt_strip;

use crate::domain::Cpt;
use axes::LinearAxis;

// A4 portrait at 72 dpi (PDF points). The SVG is laid out in these units
// so the rasterised PNG matches the page exactly when embedded full-width.
const W: f64 = 595.0;
const H: f64 = 841.0;

/// Vertical gridline + label positions for the qc (Conusweerstand) axis,
/// in MPa. Used by both `build_grid` (lines) and the qc axis row in
/// `build_header` (labels) so the labels sit *on* the gridlines.
const QC_TICKS: &[u32] = &[1, 5, 10, 15, 20, 25, 30];

// Outer printable area (page border).
const BORDER_M: f64 = 24.0;
const BORDER_W: f64 = W - 2.0 * BORDER_M;
const BORDER_H: f64 = H - 2.0 * BORDER_M;

// Header strip (three x-axis scales stacked) at the top of the printable area.
const HEADER_H: f64 = 40.0;

// Metadata footer (project info block + company line).
const FOOTER_H: f64 = 100.0;

// Plot area = the rectangle between header and footer, inside the border.
const PLOT_LEFT_M: f64 = 38.0;   // space for depth-axis labels (left of plot)
const PLOT_RIGHT_M: f64 = 14.0;

/// Render the complete A4 CPT page as SVG.
pub fn render_cpt_svg(cpt: &Cpt) -> String {
    render_cpt_svg_with_meta(cpt, None, None)
}

/// Render with optional override of project number / sondering id displayed
/// in the metadata footer. (Used by `report.rs` so the user's `ProjectMeta`
/// shows up rather than only what was in the GEF.)
pub fn render_cpt_svg_with_meta(
    cpt: &Cpt,
    project_number_override: Option<&str>,
    client_override: Option<&str>,
) -> String {
    if cpt.points.is_empty() {
        return empty_svg();
    }

    let plot_x = BORDER_M + PLOT_LEFT_M;
    let plot_y = BORDER_M + HEADER_H;
    let plot_w = BORDER_W - PLOT_LEFT_M - PLOT_RIGHT_M;
    let plot_h = BORDER_H - HEADER_H - FOOTER_H;

    // ── Depth axis (NAP, downwards-negative) ─────────────────────────────
    let z0 = cpt.metadata.ground_level_nap.unwrap_or(0.0);
    let max_depth = cpt.points.iter().map(|p| p.depth).fold(0.0_f64, f64::max);
    let z_top = z0;                                  // NAP at maaiveld
    let z_bot = z0 - max_depth.ceil();               // round down 1 m
    // axis goes from z_top (at plot_y top) down to z_bot (at plot_y+plot_h bottom)
    let z_axis = LinearAxis { min: z_top, max: z_bot, px_start: plot_y, px_end: plot_y + plot_h };

    // Robertson SBT colour strip lives inside the plot, flush against the
    // right border. ~10pt wide (≈ 3.5 mm). The Rf band is shifted left by
    // SBT_W + a small gap so the inverted Rf scale doesn't get clipped.
    const SBT_W: f64 = 10.0;
    const SBT_GAP: f64 = 2.0;
    let sbt_x = plot_x + plot_w - SBT_W;

    // Fixed reference x-scales for the three curves.
    // Rf is drawn in the right 1/5 of the plot (≈ 5x narrower than qc/fs)
    // — matches the Dutch reference plot where the wrijvingsgetal is a
    // small inverted scale in the top-right corner. With the SBT strip
    // claiming the rightmost ~10pt, Rf shifts a few pt to the left.
    let rf_band_w = plot_w * 0.20;
    let rf_band_x0 = sbt_x - SBT_GAP - rf_band_w;
    let qc_axis = LinearAxis { min: 0.0,  max: 30.0, px_start: plot_x, px_end: sbt_x - SBT_GAP };
    let fs_axis = LinearAxis { min: 0.0,  max: 0.20, px_start: plot_x, px_end: sbt_x - SBT_GAP };
    let rf_axis = LinearAxis { min: 10.0, max: 0.0,  px_start: rf_band_x0, px_end: rf_band_x0 + rf_band_w };

    // ── Build curves as polylines (against NAP depth) ────────────────────
    let qc_points = curve_points(cpt, &qc_axis, &z_axis, |p| p.qc, z0);
    let fs_points = curve_points(cpt, &fs_axis, &z_axis, |p| p.fs, z0);
    let rf_points = curve_points(cpt, &rf_axis, &z_axis, |p| p.rf, z0);

    // ── Grid ─────────────────────────────────────────────────────────────
    let grid = build_grid(plot_x, plot_y, plot_w, plot_h, z_top, z_bot, &qc_axis);

    // ── Header (3 stacked x-axes) ────────────────────────────────────────
    let header = build_header(plot_x, BORDER_M, plot_w, HEADER_H);

    // ── Depth axis labels (NAP) ──────────────────────────────────────────
    let depth_labels = build_depth_labels(plot_x, plot_y, plot_h, z_top, z_bot, &z_axis);

    // ── Footer (metadata block) ──────────────────────────────────────────
    let footer = build_footer(cpt, project_number_override, client_override);

    // ── SBT strip — vertical Robertson colour band on the right edge ─────
    // Reuses the in-app classification per measurement point and draws a
    // colored rect per consecutive same-zone band. Falls back to silent
    // skip if the CPT has no classifiable points.
    let sbt = build_sbt_strip(sbt_x, plot_y, SBT_W, plot_h, cpt, z_top, z_bot);

    // ── Maaiveld arrow (MV ↓) just inside the plot top, near the depth axis.
    let mv = format!(
        r##"<g font-family="Arial, sans-serif" font-size="9" font-weight="700" fill="#000">
  <text x="{tx:.1}" y="{ty:.1}" text-anchor="middle">MV</text>
  <path d="M {ax:.1} {ay:.1} l -3 -5 h 6 z" fill="#000" />
</g>"##,
        tx = plot_x + 14.0,
        ty = plot_y + 8.0,
        ax = plot_x + 14.0,
        ay = plot_y + 13.0,
    );

    // ── Hatched ground-level band along the plot top edge ────────────
    // A row of short slanted lines, mimicking the cartographic "ground"
    // hatch visible in the reference plot just below the header.
    let hatch = {
        let mut h = String::new();
        let band_top = plot_y + 1.0;
        let band_bot = plot_y + 7.0;
        // Skip the first 30pt so the MV arrow remains legible.
        let start = plot_x + 30.0;
        let end = plot_x + plot_w;
        let mut x = start;
        while x < end {
            h.push_str(&format!(
                r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#000" stroke-width="0.45" />"##,
                x, band_bot, x + 4.0, band_top
            ));
            x += 4.0;
        }
        h
    };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="Arial, sans-serif">
<defs>
  <clipPath id="plotClip">
    <rect x="{plot_x:.1}" y="{plot_y:.1}" width="{plot_w:.1}" height="{plot_h:.1}" />
  </clipPath>
</defs>
<rect x="0" y="0" width="{W}" height="{H}" fill="#FFFFFF" />
<rect x="{bx:.1}" y="{by:.1}" width="{bw:.1}" height="{bh:.1}" fill="none" stroke="#000" stroke-width="1.5" />
{header}
{grid}
{sbt}
{depth_labels}
{hatch}
{mv}
<g clip-path="url(#plotClip)">
<polyline points="{fs_points}" fill="none" stroke="#D02828" stroke-width="0.55" stroke-linejoin="round" stroke-linecap="round" />
<polyline points="{qc_points}" fill="none" stroke="#1F4FA8" stroke-width="0.55" stroke-linejoin="round" stroke-linecap="round" />
<polyline points="{rf_points}" fill="none" stroke="#000000" stroke-width="0.4"  stroke-linejoin="round" stroke-linecap="round" />
</g>
{footer}
</svg>"##,
        bx = BORDER_M,
        by = BORDER_M,
        bw = BORDER_W,
        bh = BORDER_H,
    )
}

fn empty_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}">
<text x="{cx}" y="{cy}" text-anchor="middle" font-family="Inter" font-size="14" fill="#888">No data</text>
</svg>"##,
        cx = W / 2.0,
        cy = H / 2.0,
    )
}

// ─── Curves ────────────────────────────────────────────────────────────

fn curve_points<F>(cpt: &Cpt, x_axis: &LinearAxis, z_axis: &LinearAxis, value: F, z0: f64) -> String
where F: Fn(&crate::domain::MeasurementPoint) -> Option<f64>
{
    let mut s = String::new();
    for p in &cpt.points {
        if let Some(v) = value(p) {
            let x = x_axis.project(v);
            // Depth → NAP: depth is positive downwards, NAP at point = z0 - depth
            let z_at = z0 - p.depth;
            let y = z_axis.project(z_at);
            if !s.is_empty() { s.push(' '); }
            s.push_str(&format!("{:.2},{:.2}", x, y));
        }
    }
    s
}

// ─── Grid ──────────────────────────────────────────────────────────────

fn build_grid(plot_x: f64, plot_y: f64, plot_w: f64, plot_h: f64, z_top: f64, z_bot: f64, qc_axis: &LinearAxis) -> String {
    let mut s = String::new();

    // Horizontal gridlines every 1 m NAP — light grey so the curves dominate.
    let z_min = z_top.min(z_bot);
    let z_max = z_top.max(z_bot);
    let mut z = z_min.ceil();
    while z <= z_max {
        let y = z_axis_proj(z_top, z_bot, plot_y, plot_h, z);
        s.push_str(&format!(
            r##"<line x1="{:.1}" y1="{:.2}" x2="{:.1}" y2="{:.2}" stroke="#B0B0B0" stroke-width="0.2" />"##,
            plot_x, y, plot_x + plot_w, y
        ));
        z += 1.0;
    }

    // Vertical gridlines at the same qc positions as the labels in the
    // header axis row, so labels sit *on* the gridlines.
    for v in QC_TICKS {
        let x = qc_axis.project(*v as f64);
        s.push_str(&format!(
            r##"<line x1="{:.2}" y1="{:.1}" x2="{:.2}" y2="{:.1}" stroke="#000000" stroke-width="0.5" />"##,
            x, plot_y, x, plot_y + plot_h
        ));
    }

    // Plot box outline
    s.push_str(&format!(
        r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="none" stroke="#000" stroke-width="0.7" />"##,
        plot_x, plot_y, plot_w, plot_h
    ));

    s
}

fn z_axis_proj(z_top: f64, z_bot: f64, py: f64, ph: f64, z: f64) -> f64 {
    let range = z_bot - z_top;
    if range.abs() < f64::EPSILON { return py; }
    let t = (z - z_top) / range;
    py + t * ph
}

// ─── SBT colour strip (inline consecutive-merge) ───────────────────────
//
// Walks the CPT's points, classifies each (qc, Rf) with `robertson::classify`,
// and emits one SVG `<rect>` per consecutive same-zone band. The result is
// stacked top-to-bottom against the plot's NAP axis, matching the Dutch
// reference report layout (and the in-app chart's soil strip).
fn build_sbt_strip(
    x: f64,
    plot_y: f64,
    width: f64,
    plot_h: f64,
    cpt: &crate::domain::Cpt,
    z_top: f64,
    z_bot: f64,
) -> String {
    use crate::robertson::classify;

    let mut out = String::new();
    let mut band_start_depth: Option<f64> = None;
    let mut band_zone: Option<crate::robertson::Zone> = None;
    let mut band_last_depth: f64 = 0.0;

    let flush = |out: &mut String, start: f64, end: f64, zone: crate::robertson::Zone| {
        // Translate depth (positive downwards from ground) into NAP, then
        // project onto the plot's vertical axis. Same maths as `curve_points`.
        let z0 = z_top; // z_top == ground level NAP (set by caller)
        let z_at_top = z0 - start;
        let z_at_bot = z0 - end;
        let y1 = z_axis_proj(z_top, z_bot, plot_y, plot_h, z_at_top).max(plot_y);
        let y2 = z_axis_proj(z_top, z_bot, plot_y, plot_h, z_at_bot).min(plot_y + plot_h);
        let h = (y2 - y1).max(0.0);
        if h <= 0.0 { return; }
        out.push_str(&format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" />"#,
            x, y1, width, h, zone.color
        ));
    };

    for p in &cpt.points {
        let qc = match p.qc { Some(v) => v, None => continue };
        let rf = match p.rf { Some(v) => v, None => continue };
        let z = match classify(qc, rf) { Some(z) => z, None => continue };
        let d = p.depth;

        match band_zone {
            None => {
                band_zone = Some(z);
                band_start_depth = Some(d);
                band_last_depth = d;
            }
            Some(curr) if curr.number == z.number => {
                band_last_depth = d;
            }
            Some(curr) => {
                flush(&mut out, band_start_depth.unwrap_or(d), band_last_depth, curr);
                band_zone = Some(z);
                band_start_depth = Some(d);
                band_last_depth = d;
            }
        }
    }
    if let (Some(zone), Some(start)) = (band_zone, band_start_depth) {
        flush(&mut out, start, band_last_depth, zone);
    }

    // Thin black border around the strip so it reads as its own column.
    out.push_str(&format!(
        r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="#000" stroke-width="0.5" />"##,
        x, plot_y, width, plot_h
    ));

    out
}

// ─── Header (three stacked x-axes) ─────────────────────────────────────

fn build_header(x: f64, y: f64, w: f64, h: f64) -> String {
    // Three axis rows stacked top→bottom (fs RED, qc BLUE, Rf BLACK inverted).
    let row_h = h / 3.0;
    let mut s = String::new();

    // Build per-row tick lists. Skip the leftmost few fs labels so the
    // "Plaatselijke wrijving (MPa)" label has room.
    let fs_ticks: Vec<(f64, String)> = (2..=10)
        .map(|i| (i as f64 * 0.02 / 0.20, format!("{:.2}", i as f64 * 0.02)))
        .collect();
    let qc_ticks: Vec<(f64, String)> = QC_TICKS
        .iter()
        .map(|v| (*v as f64 / 30.0, v.to_string()))
        .collect();
    // Rf row is rendered in the narrow right-band (≈ 1/5 of plot width),
    // so we only show the round-number ticks (10, 5, 0) — anything denser
    // would overlap with the "Wrijvingsgetal (%)" label.
    let rf_ticks: Vec<(f64, String)> = [10, 5, 0]
        .iter()
        .map(|i| ((10.0 - *i as f64) / 10.0, i.to_string()))
        .collect();

    s.push_str(&render_axis_row(x, y + 0.0 * row_h, w, row_h, "#D02828",
        "Plaatselijke wrijving (MPa)", false, &fs_ticks));
    s.push_str(&render_axis_row(x, y + 1.0 * row_h, w, row_h, "#1F4FA8",
        "Conusweerstand (MPa)", false, &qc_ticks));
    // Rf header is rendered in the right 1/5 of the plot width so its scale
    // matches the (narrowed) Rf curve band. Right-aligned label.
    let rf_band_w = w * 0.20;
    let rf_band_x = x + w - rf_band_w;
    s.push_str(&render_axis_row(rf_band_x, y + 2.0 * row_h, rf_band_w, row_h, "#000000",
        "Wrijvingsgetal (%)", true, &rf_ticks));

    s
}

fn render_axis_row(
    x: f64, row_top: f64, w: f64, row_h: f64,
    color: &str, label: &str, label_right: bool,
    ticks: &[(f64, String)],
) -> String {
    let mut s = String::new();
    let base_y = row_top + row_h - 1.0;
    let label_y = row_top + row_h * 0.62;

    // axis baseline
    s.push_str(&format!(
        r##"<line x1="{:.1}" y1="{:.2}" x2="{:.1}" y2="{:.2}" stroke="{}" stroke-width="0.45" />"##,
        x, base_y, x + w, base_y, color
    ));

    // axis label
    let (lx, ta) = if label_right { (x + w - 1.0, "end") } else { (x + 1.0, "start") };
    s.push_str(&format!(
        r##"<text x="{lx:.1}" y="{ly:.2}" font-family="Arial, sans-serif" font-size="6.5" fill="{color}" font-weight="700" text-anchor="{ta}">{label}</text>"##,
        ly = label_y,
    ));

    // tick labels + short ticks
    for (tv, lbl) in ticks {
        let tx = x + tv * w;
        s.push_str(&format!(
            r##"<text x="{tx:.2}" y="{tt:.2}" font-family="Arial, sans-serif" font-size="5.5" fill="{color}" text-anchor="middle">{lbl}</text>"##,
            tt = label_y,
        ));
        s.push_str(&format!(
            r##"<line x1="{tx:.2}" y1="{ya:.2}" x2="{tx:.2}" y2="{yb:.2}" stroke="{color}" stroke-width="0.45" />"##,
            ya = base_y - 2.5,
            yb = base_y,
        ));
    }

    s
}

// ─── Depth axis labels (NAP) ───────────────────────────────────────────

fn build_depth_labels(plot_x: f64, plot_y: f64, plot_h: f64, z_top: f64, z_bot: f64, _z_axis: &LinearAxis) -> String {
    let mut s = String::new();

    let z_min = z_top.min(z_bot);
    let z_max = z_top.max(z_bot);

    // Vertical label "DIEPTE IN METERS T.O.V. NAP" — placed left of axis, rotated.
    let mid_y = plot_y + plot_h / 2.0;
    let lbl_x = plot_x - 26.0;
    s.push_str(&format!(
        r##"<text x="{lbl_x:.1}" y="{mid_y:.1}" font-family="Arial, sans-serif" font-size="8.5" fill="#000" text-anchor="middle" font-weight="600" transform="rotate(-90 {lbl_x:.1} {mid_y:.1})">DIEPTE IN METERS T.O.V. NAP</text>"##,
    ));

    let mut z = z_min.ceil();
    while z <= z_max {
        let y = z_axis_proj(z_top, z_bot, plot_y, plot_h, z);
        s.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.2}" font-family="Inter" font-size="7" fill="#000" text-anchor="end" dominant-baseline="central">{lbl}</text>"##,
            x = plot_x - 4.0,
            lbl = format_signed(z, 0),
        ));
        z += 1.0;
    }

    s
}

// ─── Footer (metadata block) ───────────────────────────────────────────

fn build_footer(
    cpt: &Cpt,
    project_number_override: Option<&str>,
    client_override: Option<&str>,
) -> String {
    let fy0 = H - BORDER_M - FOOTER_H + 4.0;
    let fx0 = BORDER_M + 6.0;
    let mid_x = W / 2.0 - 10.0;

    // Pull metadata from cpt
    let m = &cpt.metadata;
    let extras = &m.extra;
    let opdracht_nr = project_number_override
        .map(|s| s.to_string())
        .or_else(|| m.project_number.clone())
        .or_else(|| extras.get("PROJECTID").cloned())
        .unwrap_or_default();
    let sondering = cpt.id.clone();

    // Date — try metadata.date, fall back to extras STARTDATE/FILEDATE.
    let date_str = m
        .date
        .map(|d| d.format("%d-%m-%Y").to_string())
        .unwrap_or_else(|| {
            extras.get("STARTDATE")
                .or_else(|| extras.get("FILEDATE"))
                .map(|raw| format_gef_ymd(raw))
                .unwrap_or_default()
        });
    let time_str = extras.get("STARTTIME").map(|raw| format_gef_hms(raw)).unwrap_or_default();
    let opdrachtgever = client_override
        .map(|s| s.to_string())
        .or_else(|| client_from_extras(extras))
        .unwrap_or_default();
    let omschrijving = m.project_name.clone()
        .or_else(|| extras.get("PROJECTNAME").cloned())
        .unwrap_or_default();
    let referentie_nivo = m.ground_level_nap
        .map(|z| format!("{:+.2} m t.o.v. NAP", z))
        .unwrap_or_else(|| String::from("- m t.o.v. NAP"));
    let conus_type = cone_type_from_extras(extras);
    let conus_serial = cone_serial_from_extras(extras);
    let hellingopnemer = inclinometer_from_extras(extras);
    let einde_helling = end_inclination_from_extras(cpt);

    // Layout: two columns of label/value
    let lh = 12.0;
    let mut y = fy0 + 12.0;

    let mut s = String::new();
    // Top thin separator above metadata
    s.push_str(&format!(
        r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#000" stroke-width="0.7" />"##,
        BORDER_M, fy0, BORDER_M + BORDER_W, fy0
    ));

    // Left column rows
    let left_rows: [(&str, String); 5] = [
        ("OPDRACHT NR", format!(": {}", opdracht_nr)),
        ("SONDERING",   format!(": {}", sondering)),
        ("DATUM",       format!(": {}     TIJD   : {}", date_str, time_str)),
        ("OPDRACHTGEVER", format!(": {}", opdrachtgever)),
        ("OMSCHRIJVING", format!(": {}", omschrijving)),
    ];
    for (label, value) in &left_rows {
        s.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" font-weight="700" fill="#000">{label}</text>
<text x="{xv:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" fill="#000">{value}</text>"##,
            x = fx0,
            xv = fx0 + 86.0,
            label = label,
            value = value,
        ));
        y += lh;
    }

    // Right column rows — keep only 4 base rows so OPMERKING fits within the
    // footer rectangle.  EINDWAARDE HELLING gets merged with HELLINGOPNEMER
    // line is *not* desirable; instead we widen the gap to the value column.
    let mut y = fy0 + 12.0;
    let right_rows: [(&str, String); 5] = [
        ("SONDEERMEESTER", String::from(":")),
        ("REFERENTIE NIVO", format!(": {}", referentie_nivo)),
        ("CONUS TYPE", format!(": {}     Nr.: {}", conus_type, conus_serial)),
        ("HELLINGOPNEMER", format!(": {}", hellingopnemer)),
        ("EINDWAARDE HELLING", format!(": {}", einde_helling)),
    ];
    for (label, value) in &right_rows {
        s.push_str(&format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" font-weight="700" fill="#000">{label}</text>
<text x="{xv:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" fill="#000">{value}</text>"##,
            x = mid_x,
            xv = mid_x + 110.0,
            label = label,
            value = value,
        ));
        y += lh;
    }
    // OPMERKING line — right column extra
    s.push_str(&format!(
        r##"<text x="{x:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" font-weight="700" fill="#000">OPMERKING</text>
<text x="{xv:.1}" y="{y:.1}" font-family="Arial, sans-serif" font-size="9" fill="#000">:</text>"##,
        x = mid_x,
        xv = mid_x + 110.0,
    ));

    // Bottom company line — pulled from CPT metadata, with sensible fallbacks.
    // Order: typed `equipment` (← #COMPANYID for GEF), then BRO `Bronhouder`
    // extra, then GEF #COMPANYID extra (in case the typed field is empty).
    let company = m.equipment.clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| extras.get("COMPANYID").cloned())
        .or_else(|| extras.get("Bronhouder").cloned())
        .unwrap_or_else(|| String::from("Onbekende contractor"));
    let company_y = H - BORDER_M - 6.0;
    s.push_str(&format!(
        r##"<text x="{cx:.1}" y="{y:.1}" text-anchor="middle" font-family="Arial, sans-serif" font-size="11" font-weight="700" fill="#000">{company}</text>"##,
        cx = W / 2.0,
        y = company_y,
    ));

    s
}

// ─── Metadata helpers (GEF MEASUREMENTTEXT / MEASUREMENTVAR are bundled
//     into header.extra by the parser; we pull what we need here.) ──────

fn client_from_extras(extras: &std::collections::BTreeMap<String, String>) -> Option<String> {
    extras.get("COMPANYID").cloned().or_else(|| {
        // MEASUREMENTTEXT lines: "1, GEOSONDA, Client" → " | "-joined
        extras.get("MEASUREMENTTEXT").and_then(|joined| {
            for part in joined.split('|') {
                let mut fields = part.split(',').map(|f| f.trim());
                let idx = fields.next();
                let val = fields.next();
                let _label = fields.next();
                if let (Some(i), Some(v)) = (idx, val) {
                    if i.trim() == "1" { return Some(v.to_string()); }
                }
            }
            None
        })
    })
}

fn cone_type_from_extras(extras: &std::collections::BTreeMap<String, String>) -> String {
    if let Some(joined) = extras.get("MEASUREMENTTEXT") {
        for part in joined.split('|') {
            let mut fields = part.split(',').map(|f| f.trim());
            let idx = fields.next();
            let val = fields.next();
            if let (Some(i), Some(v)) = (idx, val) {
                if i.trim() == "4" { return v.to_string(); }
            }
        }
    }
    String::new()
}

fn cone_serial_from_extras(extras: &std::collections::BTreeMap<String, String>) -> String {
    // Often embedded in cone type after a dot (e.g. S15CFII.2645). Best-effort.
    let c = cone_type_from_extras(extras);
    if let Some((_, tail)) = c.split_once('.') {
        return tail.to_string();
    }
    String::new()
}

fn inclinometer_from_extras(_extras: &std::collections::BTreeMap<String, String>) -> String {
    // No standard GEF field for inclinometer brand — leave empty.
    String::new()
}

fn end_inclination_from_extras(cpt: &Cpt) -> String {
    cpt.points
        .iter()
        .rev()
        .find_map(|p| p.inclination.map(|i| format!("{:.2}", i)))
        .unwrap_or_default()
}

fn format_gef_ymd(raw: &str) -> String {
    let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
    if parts.len() >= 3 {
        return format!("{:>2}-{:>2}-{:>4}", parts[2], parts[1], parts[0]);
    }
    raw.to_string()
}

fn format_gef_hms(raw: &str) -> String {
    let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
    if parts.len() >= 2 {
        let h = parts[0].parse::<u32>().unwrap_or(0);
        let m = parts[1].parse::<u32>().unwrap_or(0);
        return format!("{:02}:{:02}", h, m);
    }
    raw.to_string()
}

fn format_signed(v: f64, decimals: usize) -> String {
    if v == 0.0 { return format!("{:.*}", decimals, 0.0); }
    if v > 0.0 { format!("+{:.*}", decimals, v) } else { format!("{:.*}", decimals, v) }
}

// ─── PNG rasterisation ─────────────────────────────────────────────────

/// Rasterise the SVG to a PNG buffer at the given pixel width.
/// Height auto-scales from the SVG's intrinsic 595×841 aspect (≈ A4 portrait).
pub fn render_cpt_png(cpt: &Cpt) -> Vec<u8> {
    render_cpt_png_with_meta(cpt, None, None)
}

pub fn render_cpt_png_with_meta(
    cpt: &Cpt,
    project_number_override: Option<&str>,
    client_override: Option<&str>,
) -> Vec<u8> {
    let svg = render_cpt_svg_with_meta(cpt, project_number_override, client_override);
    rasterize_svg_to_png(&svg, 1600).unwrap_or_default()
}

fn rasterize_svg_to_png(svg_str: &str, target_width_px: u32) -> Option<Vec<u8>> {
    // Build options with a system-font database so text renders even when
    // the SVG asks for fonts (Inter) that may not exist on every machine —
    // resvg's `fontdb` will fall back to a sans-serif system font.
    let mut opt = resvg::usvg::Options::default();
    let mut fontdb = resvg::usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    fontdb.set_sans_serif_family("Arial");
    opt.fontdb = std::sync::Arc::new(fontdb);
    let tree = resvg::usvg::Tree::from_str(svg_str, &opt).ok()?;
    let size = tree.size();
    let scale = target_width_px as f32 / size.width();
    let pixmap_w = target_width_px;
    let pixmap_h = (size.height() * scale).ceil() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_w, pixmap_h)?;
    let ts = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Cpt, MeasurementPoint, Metadata};

    fn sample_cpt() -> Cpt {
        Cpt {
            id: "01".into(),
            metadata: Metadata { ground_level_nap: Some(-1.06), source_file: "x.gef".into(), ..Default::default() },
            position: None,
            points: (0..200).map(|i| MeasurementPoint {
                depth: i as f64 * 0.1,
                depth_nap: None,
                qc: Some(5.0 + ((i as f64) * 0.1).sin() * 4.0),
                fs: Some(0.05),
                rf: Some(1.5),
                u2: None,
                inclination: Some(0.5),
            }).collect(),
        }
    }

    #[test]
    fn renders_svg_without_panic() {
        let cpt = sample_cpt();
        let svg = render_cpt_svg(&cpt);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Conusweerstand"));
        assert!(svg.contains("Wrijvingsgetal"));
    }

    #[test]
    fn renders_png_without_panic() {
        let cpt = sample_cpt();
        let png = render_cpt_png(&cpt);
        assert!(png.starts_with(&[0x89, 0x50, 0x4E, 0x47])); // PNG magic
    }
}
