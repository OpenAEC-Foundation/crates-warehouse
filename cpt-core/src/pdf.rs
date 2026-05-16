//! Direct PDF generation for a single CPT page.
//!
//! Bypasses the openaec engine to put a *single* full-bleed CPT chart on one
//! A4 page — matching the Dutch reference layout where the chart fills the
//! entire printable area. Use [`generate_single_cpt_pdf_bytes`] for one CPT.
//!
//! Multi-CPT projects keep using [`crate::build_report`] + the openaec
//! engine (cover page + coordinate table + per-CPT pages).
//!
//! Page order produced here (single-CPT path):
//!   1. **Cover** — OpenAEC-branded title page (Deep Forge top 2/3, white
//!      bottom 1/3, amber gradient strip, project metadata block).
//!   2. **Chart** — full-bleed A4 CPT chart raster.
//!   3. **Back cover** — minimal OpenAEC closing page (Deep Forge, centred
//!      wordmark + tagline + repo link).

use printpdf::{BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocumentReference,
    PdfLayerReference, PdfPageIndex, Point, Polygon, Px, Rgb};

use crate::domain::Cpt;
use crate::plot::render_cpt_png_with_meta;
use crate::report::ProjectMeta;

// ── Brand colors (OpenAEC DESIGN-SYSTEM.md §2.1) ────────────────────
const DEEP_FORGE: (f32, f32, f32) = (54.0 / 255.0, 54.0 / 255.0, 62.0 / 255.0);
const AMBER: (f32, f32, f32) = (217.0 / 255.0, 119.0 / 255.0, 6.0 / 255.0);
const WARM_GOLD: (f32, f32, f32) = (245.0 / 255.0, 158.0 / 255.0, 11.0 / 255.0);
const SIGNAL_ORANGE: (f32, f32, f32) = (234.0 / 255.0, 88.0 / 255.0, 12.0 / 255.0);
const BLUEPRINT_WHITE: (f32, f32, f32) = (250.0 / 255.0, 250.0 / 255.0, 249.0 / 255.0);
const SCAFFOLD_GRAY: (f32, f32, f32) = (161.0 / 255.0, 161.0 / 255.0, 170.0 / 255.0);
const DEEP_FORGE_TEXT: (f32, f32, f32) = (54.0 / 255.0, 54.0 / 255.0, 62.0 / 255.0);

const A4_W_MM: f32 = 210.0;
const A4_H_MM: f32 = 297.0;

/// Generate a PDF for a single CPT: branded cover, chart, back cover.
pub fn generate_single_cpt_pdf_bytes(cpt: &Cpt, project: &ProjectMeta) -> Vec<u8> {
    let png = render_cpt_png_with_meta(
        cpt,
        Some(&project.project_number),
        Some(&project.client),
    );

    let (doc, cover_page, cover_layer) = printpdf::PdfDocument::new(
        format!("Sondering {} — {}", cpt.id, project.title),
        Mm(A4_W_MM),
        Mm(A4_H_MM),
        "Cover",
    );

    // Built-in fonts — Helvetica/HelveticaBold approximate Inter/Space
    // Grotesk closely enough for a portable, font-free PDF (no embedded
    // TTF bytes, no licensing concerns).
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .expect("HelveticaBold is always available");
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("Helvetica is always available");

    // ── 1. Cover page ──
    render_cover_page(&doc, cover_page, &cover_layer, cpt, project, &font_bold, &font_regular);

    // ── 2. Chart page ──
    let (chart_page, chart_layer) = doc.add_page(Mm(A4_W_MM), Mm(A4_H_MM), "Chart");
    render_chart_page(&doc, chart_page, &chart_layer, &png);

    // ── 3. Back cover ──
    let (back_page, back_layer) = doc.add_page(Mm(A4_W_MM), Mm(A4_H_MM), "BackCover");
    render_back_cover(&doc, back_page, &back_layer, &font_bold, &font_regular);

    doc.save_to_bytes().unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────
// Cover page
// ─────────────────────────────────────────────────────────────────────

fn render_cover_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    cpt: &Cpt,
    project: &ProjectMeta,
    font_bold: &IndirectFontRef,
    font_regular: &IndirectFontRef,
) {
    let layer = doc.get_page(page).get_layer(*layer_id);

    // Deep Forge background covers top two-thirds (0..198mm from bottom is
    // *not* the dark band — printpdf y is from the bottom-left. So the
    // dark band is y = 99mm..297mm). Bottom third stays white.
    let dark_band_y_min = A4_H_MM / 3.0; // 99 mm from bottom
    fill_rect(
        &layer,
        0.0, dark_band_y_min,
        A4_W_MM, A4_H_MM - dark_band_y_min,
        DEEP_FORGE,
    );

    // Amber gradient strip at the boundary (height ~6mm). Approximate
    // the 3-stop CSS gradient with 12 vertical bands.
    let gradient_height = 6.0_f32;
    let gradient_y = dark_band_y_min - gradient_height; // sits *under* the dark band
    draw_amber_gradient_strip(&layer, 0.0, gradient_y, A4_W_MM, gradient_height);

    // Small OpenAEC wordmark top-left in amber on dark.
    // "Open" + "AEC" in amber, all caps.
    layer.set_fill_color(rgb(AMBER));
    layer.use_text(
        "OpenAEC Foundation",
        11.0,
        Mm(14.0),
        Mm(A4_H_MM - 14.0),
        font_bold,
    );
    // Section label (JetBrains Mono in spec — we use HelveticaBold caps).
    layer.set_fill_color(rgb(WARM_GOLD));
    layer.use_text(
        "01 — SONDERINGSRAPPORT",
        7.5,
        Mm(14.0),
        Mm(A4_H_MM - 19.0),
        font_bold,
    );

    // Big title — project.title in white, centred horizontally, vertically
    // centred within the dark band.
    let title_text = project.title.clone();
    let title_font_size = 28.0_f32;
    let title_y = (A4_H_MM - 18.0).min(A4_H_MM - 60.0); // ≈ top third of dark band
    layer.set_fill_color(rgb(BLUEPRINT_WHITE));
    let title_width_est = estimate_text_width(&title_text, title_font_size, /* bold = */ true);
    let title_x = ((A4_W_MM - title_width_est) / 2.0).max(14.0);
    layer.use_text(&title_text, title_font_size, Mm(title_x), Mm(title_y - 60.0), font_bold);

    // Subtitle — "Grondonderzoek — <location>" in scaffold-gray under the title.
    let location = if project.location.is_empty() {
        "Sondering".to_string()
    } else {
        project.location.clone()
    };
    let subtitle = format!("Grondonderzoek — {}", location);
    let subtitle_size = 12.0_f32;
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    let subtitle_width = estimate_text_width(&subtitle, subtitle_size, /* bold = */ false);
    let subtitle_x = ((A4_W_MM - subtitle_width) / 2.0).max(14.0);
    layer.use_text(&subtitle, subtitle_size, Mm(subtitle_x), Mm(title_y - 72.0), font_regular);

    // ── Bottom third: project metadata table on white ──
    let meta_y_top = dark_band_y_min - gradient_height - 14.0; // start ~20mm under gradient
    let meta_label_x = 24.0_f32;
    let meta_value_x = 76.0_f32;
    let line_height = 9.0_f32;
    let label_size = 9.0_f32;
    let value_size = 12.0_f32;

    let rows: [(&str, String); 6] = [
        ("Opdrachtgever", project.client.clone()),
        ("Locatie", project.location.clone()),
        ("Projectnummer", project.project_number.clone()),
        ("Datum", project.date.format("%d-%m-%Y").to_string()),
        ("Auteur", project.author.clone()),
        ("Sondering", cpt.id.clone()),
    ];

    for (i, (label, value)) in rows.iter().enumerate() {
        let y = meta_y_top - (i as f32) * line_height;
        layer.set_fill_color(rgb(SCAFFOLD_GRAY));
        layer.use_text(*label, label_size, Mm(meta_label_x), Mm(y), font_regular);
        layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
        let display_value = if value.is_empty() { "—".to_string() } else { value.clone() };
        layer.use_text(display_value, value_size, Mm(meta_value_x), Mm(y), font_bold);
    }

    // Slim amber gradient strip at the very bottom of the page.
    let footer_strip_h = 4.0_f32;
    draw_amber_gradient_strip(&layer, 0.0, footer_strip_h, A4_W_MM, footer_strip_h);
    // Tagline above the strip in deep-forge.
    layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
    layer.use_text(
        "Build free. Build together.",
        9.0,
        Mm(14.0),
        Mm(footer_strip_h + 6.0),
        font_regular,
    );
    // Page marker right-aligned.
    let pagetxt = "Sonderingsrapport · pagina 1";
    let pw = estimate_text_width(pagetxt, 7.5, false);
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(pagetxt, 7.5, Mm(A4_W_MM - 14.0 - pw), Mm(footer_strip_h + 6.5), font_regular);
}

// ─────────────────────────────────────────────────────────────────────
// Chart page (existing full-bleed image)
// ─────────────────────────────────────────────────────────────────────

fn render_chart_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    png: &[u8],
) {
    let layer = doc.get_page(page).get_layer(*layer_id);
    if let Ok(dynamic_img) = ::image::load_from_memory(png) {
        let rgb_img = dynamic_img.to_rgb8();
        let (w, h) = (rgb_img.width() as usize, rgb_img.height() as usize);
        let raw_pixels = rgb_img.into_raw();
        let image_xobj = printpdf::ImageXObject {
            width: Px(w),
            height: Px(h),
            color_space: printpdf::ColorSpace::Rgb,
            bits_per_component: printpdf::ColorBits::Bit8,
            interpolate: true,
            image_data: raw_pixels,
            image_filter: None,
            smask: None,
            clipping_bbox: None,
        };
        let pdf_image = printpdf::Image::from(image_xobj);
        let dpi = 72.0_f32;
        let page_w_pt = A4_W_MM * 72.0 / 25.4;
        let page_h_pt = A4_H_MM * 72.0 / 25.4;
        let transform = printpdf::ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            scale_x: Some(page_w_pt / w as f32),
            scale_y: Some(page_h_pt / h as f32),
            dpi: Some(dpi),
            ..Default::default()
        };
        pdf_image.add_to_layer(layer, transform);
    }
}

// ─────────────────────────────────────────────────────────────────────
// Back cover
// ─────────────────────────────────────────────────────────────────────

fn render_back_cover(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    font_bold: &IndirectFontRef,
    font_regular: &IndirectFontRef,
) {
    let layer = doc.get_page(page).get_layer(*layer_id);

    // Full deep-forge background.
    fill_rect(&layer, 0.0, 0.0, A4_W_MM, A4_H_MM, DEEP_FORGE);

    // Centred OpenAEC monogram — minimal hexagonal block in amber strokes.
    // We draw a stylised 3D cube outline by hand (no embedded SVG path),
    // matching the brandbook "isometric open building block" symbol.
    draw_openaec_symbol(&layer, A4_W_MM / 2.0, A4_H_MM / 2.0 + 25.0, 18.0);

    // Wordmark "Open Geotechniek Studio" in Space Grotesk Bold 24pt (Helvetica
    // stand-in), centred under the symbol in white.
    let wordmark = "Open Geotechniek Studio";
    let wm_size = 22.0_f32;
    let wm_w = estimate_text_width(wordmark, wm_size, true);
    layer.set_fill_color(rgb(BLUEPRINT_WHITE));
    layer.use_text(
        wordmark,
        wm_size,
        Mm((A4_W_MM - wm_w) / 2.0),
        Mm(A4_H_MM / 2.0 - 4.0),
        font_bold,
    );

    // Version line.
    let version = "v0.1.0";
    let v_size = 10.0_f32;
    let v_w = estimate_text_width(version, v_size, false);
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(
        version,
        v_size,
        Mm((A4_W_MM - v_w) / 2.0),
        Mm(A4_H_MM / 2.0 - 14.0),
        font_regular,
    );

    // Tagline in amber.
    let tagline = "Build free. Build together.";
    let t_size = 12.0_f32;
    let t_w = estimate_text_width(tagline, t_size, false);
    layer.set_fill_color(rgb(AMBER));
    layer.use_text(
        tagline,
        t_size,
        Mm((A4_W_MM - t_w) / 2.0),
        Mm(A4_H_MM / 2.0 - 30.0),
        font_regular,
    );

    // Bottom — repo URL in white at 60% opacity. printpdf doesn't expose
    // alpha cheaply, so we fake "white 60%" with mid-grey.
    let url = "https://github.com/OpenAEC-Foundation";
    let u_size = 9.0_f32;
    let u_w = estimate_text_width(url, u_size, false);
    layer.set_fill_color(rgb((153.0 / 255.0, 153.0 / 255.0, 158.0 / 255.0)));
    layer.use_text(url, u_size, Mm((A4_W_MM - u_w) / 2.0), Mm(16.0), font_regular);

    // Bottom amber gradient strip.
    draw_amber_gradient_strip(&layer, 0.0, 4.0, A4_W_MM, 4.0);
}

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

fn rgb(c: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb::new(c.0, c.1, c.2, None))
}

/// Filled rectangle in mm coordinates from bottom-left.
fn fill_rect(
    layer: &PdfLayerReference,
    x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32,
    color: (f32, f32, f32),
) {
    layer.set_fill_color(rgb(color));
    layer.set_outline_color(rgb(color));
    let points = vec![
        (Point::new(Mm(x_mm), Mm(y_mm)), false),
        (Point::new(Mm(x_mm + w_mm), Mm(y_mm)), false),
        (Point::new(Mm(x_mm + w_mm), Mm(y_mm + h_mm)), false),
        (Point::new(Mm(x_mm), Mm(y_mm + h_mm)), false),
    ];
    let poly = Polygon {
        rings: vec![points],
        mode: printpdf::path::PaintMode::Fill,
        winding_order: printpdf::path::WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
}

/// Approximate the spec's CSS amber gradient
/// `linear-gradient(90deg, #D97706, #F59E0B, #EA580C)` with 24 vertical
/// bands, each painted as a separate solid rect. Looks like a smooth
/// gradient at any zoom.
fn draw_amber_gradient_strip(
    layer: &PdfLayerReference,
    x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32,
) {
    let bands = 24usize;
    let band_w = w_mm / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let color = interpolate_3stop(AMBER, WARM_GOLD, SIGNAL_ORANGE, t);
        fill_rect(layer, x_mm + (i as f32) * band_w, y_mm, band_w + 0.1, h_mm, color);
    }
}

/// Three-stop linear interpolation. `t` in [0,1]: 0..0.4 = a→b, 0.4..1 = b→c.
fn interpolate_3stop(
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    t: f32,
) -> (f32, f32, f32) {
    if t <= 0.4 {
        let u = t / 0.4;
        lerp_rgb(a, b, u)
    } else {
        let u = (t - 0.4) / 0.6;
        lerp_rgb(b, c, u)
    }
}

fn lerp_rgb(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

/// Rough average glyph advance for Helvetica/HelveticaBold. Good enough
/// for centring titles without measuring the embedded font metrics.
fn estimate_text_width(text: &str, font_size_pt: f32, bold: bool) -> f32 {
    // Empirically: Helvetica avg advance ≈ 0.5 × font_size in pt, bold ≈ 0.55.
    let advance_factor = if bold { 0.55 } else { 0.50 };
    let width_pt = text.chars().count() as f32 * font_size_pt * advance_factor;
    // Convert pt → mm (1 pt = 25.4/72 mm).
    width_pt * 25.4 / 72.0
}

/// Draw the OpenAEC isometric "open building block" symbol — an outlined
/// 3D cube with the front-facing face removed, evoking the open-source
/// architecture mark from the brandbook. Centred at (cx_mm, cy_mm),
/// `size_mm` is the half-edge length.
fn draw_openaec_symbol(layer: &PdfLayerReference, cx_mm: f32, cy_mm: f32, size_mm: f32) {
    let s = size_mm;
    // Cube vertices (8) in isometric projection. The "open" face is the
    // front-bottom — we draw the top, left, and right faces as outlines.

    // 4 top-face vertices (rhombus)
    let top_back  = (cx_mm,        cy_mm + s * 0.95);
    let top_left  = (cx_mm - s,    cy_mm + s * 0.45);
    let top_right = (cx_mm + s,    cy_mm + s * 0.45);
    let top_front = (cx_mm,        cy_mm + s * -0.05);

    // 4 bottom-face vertices (also a rhombus, shifted down)
    let bot_back  = (cx_mm,        cy_mm + s * 0.45);
    let bot_left  = (cx_mm - s,    cy_mm - s * 0.50);
    let bot_right = (cx_mm + s,    cy_mm - s * 0.50);

    layer.set_outline_color(rgb(AMBER));
    layer.set_outline_thickness(2.5);

    // Top face (filled with amber 15% — fake with a near-transparent
    // amber via solid mid-tone since printpdf 0.7 has no rgba).
    layer.set_fill_color(rgb((220.0 / 255.0, 160.0 / 255.0, 70.0 / 255.0)));
    let top_face = Polygon {
        rings: vec![vec![
            (Point::new(Mm(top_back.0), Mm(top_back.1)), false),
            (Point::new(Mm(top_left.0), Mm(top_left.1)), false),
            (Point::new(Mm(top_front.0), Mm(top_front.1)), false),
            (Point::new(Mm(top_right.0), Mm(top_right.1)), false),
        ]],
        mode: printpdf::path::PaintMode::FillStroke,
        winding_order: printpdf::path::WindingOrder::NonZero,
    };
    layer.add_polygon(top_face);

    // Left face — outline only, faintly tinted.
    let left_face = Polygon {
        rings: vec![vec![
            (Point::new(Mm(top_left.0), Mm(top_left.1)), false),
            (Point::new(Mm(bot_left.0), Mm(bot_left.1)), false),
            (Point::new(Mm(bot_back.0), Mm(bot_back.1)), false),
            (Point::new(Mm(top_front.0), Mm(top_front.1)), false),
        ]],
        mode: printpdf::path::PaintMode::Stroke,
        winding_order: printpdf::path::WindingOrder::NonZero,
    };
    layer.add_polygon(left_face);

    // Right face — outline only.
    let right_face = Polygon {
        rings: vec![vec![
            (Point::new(Mm(top_right.0), Mm(top_right.1)), false),
            (Point::new(Mm(bot_right.0), Mm(bot_right.1)), false),
            (Point::new(Mm(bot_back.0), Mm(bot_back.1)), false),
            (Point::new(Mm(top_front.0), Mm(top_front.1)), false),
        ]],
        mode: printpdf::path::PaintMode::Stroke,
        winding_order: printpdf::path::WindingOrder::NonZero,
    };
    layer.add_polygon(right_face);

    // A small horizontal "open" notch line to signal the missing front
    // face (the visible cut-out at bottom front of the cube).
    let notch = Line {
        points: vec![
            (Point::new(Mm(bot_left.0 + s * 0.4), Mm(bot_left.1 + s * 0.15)), false),
            (Point::new(Mm(bot_right.0 - s * 0.4), Mm(bot_right.1 + s * 0.15)), false),
        ],
        is_closed: false,
    };
    layer.add_line(notch);
}
