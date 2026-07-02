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

/// Generate a PDF for a single CPT met de standaard-secties (gebrand
/// voorblad → schermvullende grafiek → achterblad).
pub fn generate_single_cpt_pdf_bytes(cpt: &Cpt, project: &ProjectMeta) -> Vec<u8> {
    generate_single_cpt_pdf_bytes_with_sections(
        cpt,
        project,
        crate::report::ReportSections::default(),
        None,
    )
}

/// Als boven, maar met expliciete sectie-selectie. Het GEBRANDE rapport
/// blijft het frame (voorblad + schermvullende grafiek + achterblad); de
/// extra secties (coördinatentabel, overzichtskaart, SBT-legenda, metadata)
/// worden als eigen pagina's ertussen geplaatst wanneer aangevinkt. Zo
/// werken de vinkjes zonder dat de gebrande lay-out verloren gaat.
pub fn generate_single_cpt_pdf_bytes_with_sections(
    cpt: &Cpt,
    project: &ProjectMeta,
    sec: crate::report::ReportSections,
    basemap: Option<&crate::report::OverviewBasemap>,
) -> Vec<u8> {
    #[derive(Clone, Copy)]
    enum Pg {
        Cover,
        CoordTable,
        Map,
        Chart,
        Legend,
        Metadata,
        Back,
    }
    let mut pages: Vec<Pg> = Vec::new();
    if sec.cover {
        pages.push(Pg::Cover);
    }
    if sec.coord_table {
        pages.push(Pg::CoordTable);
    }
    if sec.map {
        pages.push(Pg::Map);
    }
    if sec.per_cpt {
        pages.push(Pg::Chart);
    }
    if sec.sbt_legend {
        pages.push(Pg::Legend);
    }
    if sec.metadata {
        pages.push(Pg::Metadata);
    }
    if sec.cover {
        pages.push(Pg::Back);
    }
    if pages.is_empty() {
        pages.push(Pg::Chart);
    }

    // Chart-PNG alleen renderen als er daadwerkelijk een chart-pagina komt —
    // de 1600px-rasterisatie is verreweg de duurste stap; met alleen
    // coördinatentabel/metadata aangevinkt werd hij eerst weggegooid.
    let needs_chart = pages.iter().any(|p| matches!(p, Pg::Chart));
    let png = if needs_chart {
        render_cpt_png_with_meta(cpt, Some(&project.project_number), Some(&project.client))
    } else {
        Vec::new()
    };

    let (doc, first_page, first_layer) = printpdf::PdfDocument::new(
        format!("Sondering {} — {}", cpt.id, project.title),
        Mm(A4_W_MM),
        Mm(A4_H_MM),
        "Layer",
    );
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .expect("HelveticaBold is always available");
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .expect("Helvetica is always available");

    for (i, pg) in pages.iter().enumerate() {
        let (page, layer_id) = if i == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(Mm(A4_W_MM), Mm(A4_H_MM), "Layer")
        };
        match pg {
            Pg::Cover => {
                render_cover_page(&doc, page, &layer_id, cpt, project, &font_bold, &font_regular)
            }
            Pg::Chart => render_chart_page(&doc, page, &layer_id, &png),
            Pg::Back => render_back_cover(&doc, page, &layer_id, &font_bold, &font_regular),
            Pg::Map => {
                let svg = crate::report::overview_map_svg(std::slice::from_ref(cpt), basemap);
                let img = crate::plot::rasterize_svg_to_png(&svg, 1400);
                render_image_page(&doc, page, &layer_id, &font_bold, "Overzichtskaart", img.as_deref());
            }
            Pg::Legend => {
                let svg = crate::report::sbt_legend_svg();
                let img = crate::plot::rasterize_svg_to_png(&svg, 1000);
                render_image_page(
                    &doc,
                    page,
                    &layer_id,
                    &font_bold,
                    "Robertson SBT — grondsoort-legenda",
                    img.as_deref(),
                );
            }
            Pg::CoordTable => {
                render_coord_table_page(&doc, page, &layer_id, cpt, &font_bold, &font_regular)
            }
            Pg::Metadata => {
                render_metadata_page(&doc, page, &layer_id, cpt, &font_bold, &font_regular)
            }
        }
    }

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

    // ── Layout (mirrors OpenAEC-style-book/reports/Voorblad_reference.pdf)
    // Dark zone covers the top ~70 % of the page so the white "title
    // strip" at the bottom is only ~30 % tall. Badges sit at the very
    // bottom of the dark zone, just above the amber boundary strip. The
    // title fills the white strip in big Deep Forge bold.
    let dark_band_y_min = A4_H_MM * 0.30;           // ≈ 89 mm from bottom
    let dark_band_h = A4_H_MM - dark_band_y_min;
    fill_rect(&layer, 0.0, dark_band_y_min, A4_W_MM, dark_band_h, DEEP_FORGE);

    // Faint blueprint hatch over the upper third — gives the dark zone
    // some texture, mirrors the reference's hexagonal/isometric pattern
    // without needing an embedded raster.
    let pattern_y = dark_band_y_min + dark_band_h * 0.55;
    let pattern_h = dark_band_h - (pattern_y - dark_band_y_min);
    draw_blueprint_pattern(&layer, 0.0, pattern_y, A4_W_MM, pattern_h);

    // Subtle city-skyline silhouette at the bottom of the dark zone
    // (just above the badges). Rectangles of varying heights stacked
    // left-to-right at a faint warm-gold tint.
    draw_skyline(&layer, dark_band_y_min + 22.0);

    // Amber boundary strip on the dark/light divide.
    let gradient_height = 1.6_f32;
    let gradient_y = dark_band_y_min - gradient_height;
    draw_amber_gradient_strip(&layer, 0.0, gradient_y, A4_W_MM, gradient_height);

    // ── Top-left: OpenAEC wordmark + tagline ────────────────────────
    // Approximates the boxed "OpenAEC" logo in the reference by rendering
    // an amber-on-dark wordmark with a thin amber outline to its left.
    layer.set_fill_color(rgb(AMBER));
    layer.use_text(
        "OpenAEC",
        14.0,
        Mm(20.0),
        Mm(A4_H_MM - 17.0),
        font_bold,
    );
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(
        "Build free. Build together.",
        9.0,
        Mm(20.0),
        Mm(A4_H_MM - 25.5),
        font_regular,
    );
    // Small amber accent box mimicking the boxed logo to the left.
    fill_rect(&layer, 13.0, A4_H_MM - 22.0, 4.5, 7.5, AMBER);

    // ── Top-right: website link in amber ────────────────────────────
    let website = "openaec.org";
    let web_size = 11.0_f32;
    let web_w = estimate_text_width(website, web_size, false);
    layer.set_fill_color(rgb(AMBER));
    layer.use_text(
        website,
        web_size,
        Mm(A4_W_MM - 14.0 - web_w),
        Mm(A4_H_MM - 17.0),
        font_bold,
    );

    // ── Pill badges (bottom-right of dark band) ─────────────────────
    // Two pills sit centred horizontally a little above the amber strip
    // in the reference layout. We right-align them so they don't fight
    // the skyline silhouette on the left.
    let badge_h_mm = 8.5_f32;
    let badge_y_mm = dark_band_y_min + 8.0;
    let badge_pad_x = 7.0_f32;
    // Compute widths so we can right-align the pair.
    let open_w = estimate_text_width("OPEN SOURCE", 9.0, true) + 2.0 * badge_pad_x;
    let eng_w  = estimate_text_width("ENGINEERING", 9.0, true) + 2.0 * badge_pad_x;
    let gap = 3.0_f32;
    let right_edge = A4_W_MM - 18.0;
    let eng_x = right_edge - eng_w;
    let open_x = eng_x - gap - open_w;
    draw_badge(
        &layer, font_bold,
        "OPEN SOURCE", AMBER, BLUEPRINT_WHITE,
        open_x, badge_y_mm, badge_h_mm, badge_pad_x,
    );
    draw_badge(
        &layer, font_bold,
        "ENGINEERING", (42.0/255.0, 42.0/255.0, 50.0/255.0), BLUEPRINT_WHITE,
        eng_x, badge_y_mm, badge_h_mm, badge_pad_x,
    );

    // ── White zone: big project title + subtitle ────────────────────
    // Title anchored toward the bottom-left, subtitle directly under it.
    // Matches the reference where the white zone has just these two
    // elements (no metadata grid, no clutter).
    let title_text = if project.title.trim().is_empty() {
        format!("Sondering {}", cpt.id)
    } else {
        project.title.clone()
    };
    let title_size = 36.0_f32;
    let title_y = 38.0_f32;
    layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
    layer.use_text(&title_text, title_size, Mm(20.0), Mm(title_y), font_bold);

    // Subtitle — "Sonderingsrapport — <location>" in scaffold gray.
    let subtitle = if project.location.is_empty() {
        format!("Sonderingsrapport · sondering {}", cpt.id)
    } else {
        format!("Sonderingsrapport · {}", project.location)
    };
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(&subtitle, 13.0, Mm(20.0), Mm(title_y - 9.0), font_regular);

    // Compact bottom-right metadata cluster — project + datum, only
    // when supplied. Keeps the cover clean while still surfacing the
    // essential project context.
    let mut meta_y = title_y + 2.0;
    let meta_x = A4_W_MM - 18.0;
    if !project.client.is_empty() {
        let s = format!("Opdrachtgever  {}", project.client);
        let w = estimate_text_width(&s, 8.5, false);
        layer.set_fill_color(rgb(SCAFFOLD_GRAY));
        layer.use_text(&s, 8.5, Mm(meta_x - w), Mm(meta_y), font_regular);
        meta_y -= 5.0;
    }
    if !project.project_number.is_empty() {
        let s = format!("Projectnr  {}", project.project_number);
        let w = estimate_text_width(&s, 8.5, false);
        layer.set_fill_color(rgb(SCAFFOLD_GRAY));
        layer.use_text(&s, 8.5, Mm(meta_x - w), Mm(meta_y), font_regular);
        meta_y -= 5.0;
    }
    let date_s = format!("Datum  {}", project.date.format("%d-%m-%Y"));
    let date_w = estimate_text_width(&date_s, 8.5, false);
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(&date_s, 8.5, Mm(meta_x - date_w), Mm(meta_y), font_regular);

    // ── Footer strip — slim amber band + tagline & page marker ──────
    let footer_strip_h = 2.5_f32;
    draw_amber_gradient_strip(&layer, 0.0, 0.0, A4_W_MM, footer_strip_h);
    layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
    layer.use_text(
        "Open Geotechniek Studio",
        9.0,
        Mm(20.0),
        Mm(footer_strip_h + 5.0),
        font_bold,
    );
    let pagetxt = "Sonderingsrapport · pagina 1";
    let pw = estimate_text_width(pagetxt, 7.5, false);
    layer.set_fill_color(rgb(SCAFFOLD_GRAY));
    layer.use_text(pagetxt, 7.5, Mm(A4_W_MM - 18.0 - pw), Mm(footer_strip_h + 5.5), font_regular);
}

/// Draw a faint city-skyline silhouette at the bottom of the dark zone.
/// Each "building" is a tall rectangle in a slightly lighter shade than
/// Deep Forge, evoking the cityscape strip in the OpenAEC cover
/// reference without any embedded artwork.
fn draw_skyline(layer: &PdfLayerReference, y_baseline: f32) {
    let band = (
        WARM_GOLD.0 * 0.25 + DEEP_FORGE.0 * 0.75,
        WARM_GOLD.1 * 0.25 + DEEP_FORGE.1 * 0.75,
        WARM_GOLD.2 * 0.25 + DEEP_FORGE.2 * 0.75,
    );
    let heights: [f32; 22] = [
        6.0, 9.0, 4.5, 7.5, 12.0, 5.5, 10.0, 8.0, 6.5, 11.0,
        4.0, 9.5, 7.0, 5.0, 13.0, 8.5, 6.0, 10.5, 5.5, 8.0, 4.5, 7.0,
    ];
    let mut x = 8.0_f32;
    let w = 9.0_f32;
    let gap = 1.0_f32;
    for h in heights {
        fill_rect(layer, x, y_baseline, w, h, band);
        x += w + gap;
        if x > A4_W_MM - 8.0 { break; }
    }
}

/// Draw a filled rounded pill with centred text. Used for the OPEN SOURCE
/// and discipline badges on the cover, matching the
/// openaec_foundation/templates/cover.yaml badge styling.
fn draw_badge(
    layer: &PdfLayerReference,
    font: &IndirectFontRef,
    text: &str,
    fill: (f32, f32, f32),
    text_color: (f32, f32, f32),
    x_mm: f32,
    y_mm: f32,
    h_mm: f32,
    pad_x: f32,
) {
    let font_size = 8.5_f32;
    let txt_w = estimate_text_width(text, font_size, true);
    let w = txt_w + 2.0 * pad_x;
    fill_rect(layer, x_mm, y_mm, w, h_mm, fill);
    // Centre text vertically (baseline approximation: 0.7 * h).
    layer.set_fill_color(rgb(text_color));
    layer.use_text(
        text,
        font_size,
        Mm(x_mm + pad_x),
        Mm(y_mm + h_mm * 0.36),
        font,
    );
}

/// Draw a subtle "blueprint" diagonal hatch over the top of the dark
/// zone. Approximates the hero illustration overlay from the spec
/// without needing an embedded raster — keeps the PDF compact and
/// font-/image-free for the single-CPT path.
fn draw_blueprint_pattern(
    layer: &PdfLayerReference,
    x_mm: f32,
    y_mm: f32,
    w_mm: f32,
    h_mm: f32,
) {
    // Faint warm-gold lines at 10° angle, ~12 mm spacing.
    layer.set_outline_color(Color::Rgb(Rgb::new(
        WARM_GOLD.0 * 0.4 + DEEP_FORGE.0 * 0.6,
        WARM_GOLD.1 * 0.4 + DEEP_FORGE.1 * 0.6,
        WARM_GOLD.2 * 0.4 + DEEP_FORGE.2 * 0.6,
        None,
    )));
    layer.set_outline_thickness(0.3);
    let step = 12.0_f32;
    let mut x = x_mm - h_mm; // start off to the left so the slope crosses the band
    while x < x_mm + w_mm {
        let p0 = Point::new(Mm(x), Mm(y_mm));
        let p1 = Point::new(Mm(x + h_mm * 1.4), Mm(y_mm + h_mm));
        let line = Line {
            points: vec![(p0, false), (p1, false)],
            is_closed: false,
        };
        layer.add_line(line);
        x += step;
    }
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
    draw_openaec_symbol(&layer, A4_W_MM / 2.0, A4_H_MM / 2.0 + 30.0, 18.0);

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
    // Correct isometrisch "open bouwblok" (2:1 dimetrie).
    // De oude versie verbond de zijvlakken met een niet-bestaand
    // "bot_back"-punt op TOP-hoogte, waardoor het symbool als een
    // scheve tent met kruisende lijnen rendere — vandaar de herbouw.
    //
    // Geometrie: bovenvlak = ruit met halve breedte `s` en halve hoogte
    // `h = s/2`; de drie zichtbare onderhoeken liggen exact `d` lager.
    let s = size_mm;
    let h = s * 0.5;
    let d = s * 0.9; // bloklengte omlaag — iets gedrongen, zoals het merk
    // Verticaal centreren rond de aangevraagde cy: het blok loopt 0.5s
    // omhoog en 1.4s omlaag vanaf de ruit-oorsprong, dus 0.45s omhoog
    // schuiven maakt boven- en onderkant symmetrisch rond cy_mm.
    let cy_mm = cy_mm + s * 0.45;

    // Bovenvlak-ruit.
    let t_back  = (cx_mm,     cy_mm + h);
    let t_left  = (cx_mm - s, cy_mm);
    let t_right = (cx_mm + s, cy_mm);
    let t_front = (cx_mm,     cy_mm - h);
    // Onderhoeken (zelfde x, exact d lager).
    let b_left  = (t_left.0,  t_left.1 - d);
    let b_right = (t_right.0, t_right.1 - d);
    let b_front = (t_front.0, t_front.1 - d);

    // Vlaktinten: printpdf 0.7 kent geen alpha, dus we mengen amber vooraf
    // met de DEEP_FORGE-achtergrond. Boven het lichtst, links donkerder,
    // rechts ertussenin — dat geeft de 3D-diepte.
    let blend = |t: f32| -> (f32, f32, f32) {
        (
            DEEP_FORGE.0 + (AMBER.0 - DEEP_FORGE.0) * t,
            DEEP_FORGE.1 + (AMBER.1 - DEEP_FORGE.1) * t,
            DEEP_FORGE.2 + (AMBER.2 - DEEP_FORGE.2) * t,
        )
    };
    let quad = |pts: [(f32, f32); 4], fill: Option<(f32, f32, f32)>| Polygon {
        rings: vec![pts
            .iter()
            .map(|p| (Point::new(Mm(p.0), Mm(p.1)), false))
            .collect()],
        mode: if fill.is_some() {
            printpdf::path::PaintMode::FillStroke
        } else {
            printpdf::path::PaintMode::Stroke
        },
        winding_order: printpdf::path::WindingOrder::NonZero,
    };

    layer.set_outline_color(rgb(AMBER));
    layer.set_outline_thickness(1.6);

    // Linkervlak (donkerste tint).
    layer.set_fill_color(rgb(blend(0.18)));
    layer.add_polygon(quad([t_left, t_front, b_front, b_left], Some(blend(0.18))));
    // Rechtervlak (middentint).
    layer.set_fill_color(rgb(blend(0.34)));
    layer.add_polygon(quad([t_right, t_front, b_front, b_right], Some(blend(0.34))));
    // Bovenvlak (lichtste tint) als laatste zodat zijn randen bovenop liggen.
    layer.set_fill_color(rgb(blend(0.55)));
    layer.add_polygon(quad([t_back, t_left, t_front, t_right], Some(blend(0.55))));

    // "Open" merkgebaar: de voorste verticale ribbe is bewust onderbroken —
    // alleen het bovenste en onderste kwart zijn getekend, het midden staat
    // open. Warm gold zodat de seam net oplicht t.o.v. de amber randen.
    layer.set_outline_color(rgb(WARM_GOLD));
    layer.set_outline_thickness(1.6);
    let seam_top = Line {
        points: vec![
            (Point::new(Mm(t_front.0), Mm(t_front.1)), false),
            (Point::new(Mm(t_front.0), Mm(t_front.1 - d * 0.25)), false),
        ],
        is_closed: false,
    };
    let seam_bottom = Line {
        points: vec![
            (Point::new(Mm(b_front.0), Mm(b_front.1 + d * 0.25)), false),
            (Point::new(Mm(b_front.0), Mm(b_front.1)), false),
        ],
        is_closed: false,
    };
    layer.add_line(seam_top);
    layer.add_line(seam_bottom);
}

// ─────────────────────────────────────────────────────────────────────
// Sectie-pagina's binnen het gebrande frame
// ─────────────────────────────────────────────────────────────────────

/// Pagina met een gecentreerde afbeelding (overzichtskaart / SBT-legenda)
/// onder een gebrande titel + amber-streep.
fn render_image_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    font_bold: &IndirectFontRef,
    title: &str,
    png: Option<&[u8]>,
) {
    let layer = doc.get_page(page).get_layer(*layer_id);
    layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
    layer.use_text(title, 20.0, Mm(20.0), Mm(A4_H_MM - 24.0), font_bold);
    draw_amber_gradient_strip(&layer, 20.0, A4_H_MM - 30.0, 60.0, 1.6);

    let png = match png {
        Some(p) => p,
        None => return,
    };
    let img = match ::image::load_from_memory(png) {
        Ok(i) => i,
        Err(_) => return,
    };
    let rgb_img = img.to_rgb8();
    let (w, h) = (rgb_img.width() as usize, rgb_img.height() as usize);
    if w == 0 || h == 0 {
        return;
    }
    let raw = rgb_img.into_raw();
    let xobj = printpdf::ImageXObject {
        width: Px(w),
        height: Px(h),
        color_space: printpdf::ColorSpace::Rgb,
        bits_per_component: printpdf::ColorBits::Bit8,
        interpolate: true,
        image_data: raw,
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    };
    let pdf_image = printpdf::Image::from(xobj);
    let max_w = 170.0_f32;
    let max_h = 215.0_f32;
    let aspect = h as f32 / w as f32;
    let mut dw = max_w;
    let mut dh = dw * aspect;
    if dh > max_h {
        dh = max_h;
        dw = dh / aspect;
    }
    let tx = (A4_W_MM - dw) / 2.0;
    let ty = (A4_H_MM - 42.0) - dh;
    let transform = printpdf::ImageTransform {
        translate_x: Some(Mm(tx)),
        translate_y: Some(Mm(ty)),
        scale_x: Some((dw * 72.0 / 25.4) / w as f32),
        scale_y: Some((dh * 72.0 / 25.4) / h as f32),
        dpi: Some(72.0),
        ..Default::default()
    };
    pdf_image.add_to_layer(layer, transform);
}

/// Tabel-pagina onder een gebrande titel. `col_x` = linker-x (mm) per kolom.
#[allow(clippy::too_many_arguments)]
fn render_table_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    font_bold: &IndirectFontRef,
    font_regular: &IndirectFontRef,
    title: &str,
    headers: &[&str],
    col_x: &[f32],
    rows: &[Vec<String>],
) {
    let layer = doc.get_page(page).get_layer(*layer_id);
    layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
    layer.use_text(title, 20.0, Mm(20.0), Mm(A4_H_MM - 24.0), font_bold);
    draw_amber_gradient_strip(&layer, 20.0, A4_H_MM - 30.0, 60.0, 1.6);

    let row_h = 9.0_f32;
    let left = 18.0_f32;
    let width = A4_W_MM - 2.0 * left;
    let mut y = A4_H_MM - 48.0;

    fill_rect(&layer, left, y - 6.0, width, row_h, DEEP_FORGE);
    layer.set_fill_color(rgb(BLUEPRINT_WHITE));
    for (i, hd) in headers.iter().enumerate() {
        if i < col_x.len() {
            layer.use_text(*hd, 9.0, Mm(col_x[i]), Mm(y - 3.5), font_bold);
        }
    }
    y -= row_h;

    for (ri, row) in rows.iter().enumerate() {
        if ri % 2 == 1 {
            fill_rect(&layer, left, y - 6.0, width, row_h, (0.96, 0.96, 0.955));
        }
        layer.set_fill_color(rgb(DEEP_FORGE_TEXT));
        for (ci, cell) in row.iter().enumerate() {
            if ci < col_x.len() {
                layer.use_text(cell, 8.5, Mm(col_x[ci]), Mm(y - 3.5), font_regular);
            }
        }
        y -= row_h;
    }
}

fn cpt_end_depth(cpt: &Cpt) -> f64 {
    cpt.points.iter().map(|p| p.depth).fold(0.0_f64, f64::max)
}

fn render_coord_table_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    cpt: &Cpt,
    font_bold: &IndirectFontRef,
    font_regular: &IndirectFontRef,
) {
    let (x, y) = match cpt.position {
        Some(p) => (format!("{:.2}", p.x_rd), format!("{:.2}", p.y_rd)),
        None => (String::new(), String::new()),
    };
    let z = cpt
        .position
        .and_then(|p| p.z_nap)
        .or(cpt.metadata.ground_level_nap)
        .map(|v| format!("{:.2}", v))
        .unwrap_or_default();
    let datum = cpt
        .metadata
        .date
        .map(|d| d.format("%d-%m-%Y").to_string())
        .unwrap_or_default();
    let rows = vec![vec![
        cpt.id.clone(),
        x,
        y,
        z,
        format!("{:.2}", cpt_end_depth(cpt)),
        datum,
    ]];
    render_table_page(
        doc,
        page,
        layer_id,
        font_bold,
        font_regular,
        "Coordinatentabel",
        &["Sondering", "X-RD [m]", "Y-RD [m]", "Z-NAP [m]", "Diepte tot [m]", "Datum"],
        &[22.0, 55.0, 88.0, 120.0, 150.0, 178.0],
        &rows,
    );
}

fn render_metadata_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer_id: &printpdf::PdfLayerIndex,
    cpt: &Cpt,
    font_bold: &IndirectFontRef,
    font_regular: &IndirectFontRef,
) {
    let mv = cpt
        .metadata
        .ground_level_nap
        .map(|z| format!("{:.2}", z))
        .unwrap_or_default();
    let datum = cpt
        .metadata
        .date
        .map(|d| d.format("%d-%m-%Y").to_string())
        .unwrap_or_default();
    let rows = vec![
        vec!["Sondering".to_string(), cpt.id.clone()],
        vec!["Project".to_string(), cpt.metadata.project_name.clone().unwrap_or_default()],
        vec!["Apparatuur".to_string(), cpt.metadata.equipment.clone().unwrap_or_default()],
        vec!["Maaiveld [m NAP]".to_string(), mv],
        vec!["Datum".to_string(), datum],
        vec!["Eindiepte [m]".to_string(), format!("{:.2}", cpt_end_depth(cpt))],
    ];
    render_table_page(
        doc,
        page,
        layer_id,
        font_bold,
        font_regular,
        "Metadata-overzicht",
        &["Veld", "Waarde"],
        &[22.0, 80.0],
        &rows,
    );
}
