//! Special pages — cover, colofon, TOC, and backcover.
//!
//! Builds DrawList-based RawPages for non-content pages.
//! These are pre-rendered canvases inserted before/after the main content.

use openaec_layout::{Color, DrawList, Pt, RawPage, A4};

use crate::brand::BrandConfig;
use crate::schema::ReportData;

// ── A4 reference scaling ─────────────────────────────────────────────

/// A4 reference width in points (595.28).
const A4_REF_W: f64 = 595.28;
/// A4 reference height in points (841.89).
const A4_REF_H: f64 = 841.89;

/// Scale an x-coordinate from A4 reference space to actual page width.
fn sx(x_ref: f64, page_w: f32) -> Pt {
    Pt((x_ref / A4_REF_W * page_w as f64) as f32)
}

/// Scale a y-coordinate from A4 reference space to actual page height.
fn sy(y_ref: f64, page_h: f32) -> Pt {
    Pt((y_ref / A4_REF_H * page_h as f64) as f32)
}

/// Scale a font size from A4 reference space (proportional to page height).
fn sf(size_ref: f64, page_h: f32) -> Pt {
    Pt((size_ref / A4_REF_H * page_h as f64) as f32)
}

/// Resolve a font name from brand config with LiberationSans fallback.
fn resolve_font(brand: &BrandConfig, role: &str) -> String {
    brand.resolve_font_name(role)
}

// ── Color helpers ────────────────────────────────────────────────────

/// Resolve the primary brand color, falling back to 3BM default.
fn brand_primary(brand: &BrandConfig) -> Color {
    brand
        .resolve_color("$primary")
        .and_then(Color::from_hex)
        .unwrap_or(Color::rgb(0x40, 0x12, 0x4A))
}

/// Resolve the secondary brand color, falling back to 3BM default.
fn brand_secondary(brand: &BrandConfig) -> Color {
    brand
        .resolve_color("$secondary")
        .and_then(Color::from_hex)
        .unwrap_or(Color::rgb(0x38, 0xBD, 0xA0))
}

/// Resolve the text brand color, falling back to 3BM default.
fn brand_text(brand: &BrandConfig) -> Color {
    brand
        .resolve_color("$text")
        .and_then(Color::from_hex)
        .unwrap_or(Color::rgb(0x45, 0x24, 0x3D))
}

// ── Cover page ───────────────────────────────────────────────────────

/// Build a cover page with brand colors.
///
/// Layout follows brand.yaml `pages.cover` spec when available.
/// Falls back to a simplified layout when no cover page config exists.
pub fn build_cover_page(data: &ReportData, brand: &BrandConfig) -> RawPage {
    let mut dl = DrawList::new();
    let primary = brand_primary(brand);
    let secondary = brand_secondary(brand);

    let page_w = A4.width;
    let page_h = A4.height;

    let heading_font = resolve_font(brand, "heading");
    let body_font = resolve_font(brand, "body");

    // ── Purple background rect ───────────────────────────────────
    let purple_y = brand.page_f64("cover", "purple_rect_y_ref", page_h.0 as f64 * 0.25);
    let purple_h = brand.page_f64("cover", "purple_rect_h_ref", page_h.0 as f64 * 0.75);
    dl.set_fill_color(primary);
    dl.draw_rect(
        Pt(0.0),
        sy(purple_y, page_h.0),
        page_w,
        sy(purple_h, page_h.0),
        true,
        false,
    );

    // ── Logo ─────────────────────────────────────────────────────
    let logo_key = brand.page_str("cover", "logo_key", "white");
    let logo_fallback = brand.page_str("cover", "logo_fallback", "main");
    if let Some(logo_path) = brand.logo_path(&logo_key).or_else(|| brand.logo_path(&logo_fallback))
    {
        if let Ok(logo_data) = std::fs::read(&logo_path) {
            let logo_x = sx(
                brand.page_f64("cover", "logo_x_ref", 62.0),
                page_w.0,
            );
            let logo_y = sy(
                brand.page_f64("cover", "logo_y_ref", 775.0),
                page_h.0,
            );
            let logo_w = sx(
                brand.page_f64("cover", "logo_w_ref", 100.0),
                page_w.0,
            );
            // Maintain aspect ratio (estimate ~35% height-to-width)
            let logo_h = Pt(logo_w.0 * 0.35);
            dl.draw_image(logo_data, logo_x, logo_y, logo_w, logo_h);
        }
    }

    // ── Title ────────────────────────────────────────────────────
    let title_size = sf(
        brand.page_f64("cover", "title_size_ref", 28.9),
        page_h.0,
    );
    let title_x = sx(
        brand.page_f64("cover", "title_x_ref", 54.28),
        page_w.0,
    );
    let title_y = sy(
        brand.page_f64("cover", "title_y_ref", 93.47),
        page_h.0,
    );
    // Title is on the white area above the purple rect → use text color
    let title_color = brand_text(brand);
    dl.set_fill_color(title_color);
    dl.set_font(&heading_font, title_size);
    dl.draw_text(title_x, title_y, &data.project);

    // ── Subtitle ─────────────────────────────────────────────────
    if let Some(ref cover) = data.cover
        && let Some(ref subtitle) = cover.subtitle
    {
        let sub_size = sf(
            brand.page_f64("cover", "subtitle_size_ref", 17.8),
            page_h.0,
        );
        let sub_x = sx(
            brand.page_f64("cover", "subtitle_x_ref", 55.0),
            page_w.0,
        );
        let sub_y = sy(
            brand.page_f64("cover", "subtitle_y_ref", 63.0),
            page_h.0,
        );
        dl.set_fill_color(secondary);
        dl.set_font(&body_font, sub_size);
        dl.draw_text(sub_x, sub_y, subtitle);
    }

    // ── "Ontdek ons" tekst ───────────────────────────────────────
    let ontdek_text = brand.page_str("cover", "ontdek_text", "");
    if !ontdek_text.is_empty() {
        let ontdek_url = brand.page_str("cover", "ontdek_url", "");
        let ontdek_size = sf(
            brand.page_f64("cover", "ontdek_size_ref", 13.0),
            page_h.0,
        );
        let ontdek_x = sx(
            brand.page_f64("cover", "ontdek_x_ref", 401.3),
            page_w.0,
        );
        let ontdek_y = sy(
            brand.page_f64("cover", "ontdek_y_ref", 788.7),
            page_h.0,
        );
        dl.set_fill_color(Color::WHITE);
        dl.set_font(&body_font, ontdek_size);
        dl.draw_text(ontdek_x, ontdek_y, &ontdek_text);

        if !ontdek_url.is_empty() {
            let url_x = sx(
                brand.page_f64("cover", "ontdek_url_x_ref", 477.9),
                page_w.0,
            );
            dl.set_fill_color(secondary);
            dl.set_font(&heading_font, ontdek_size);
            dl.draw_text(url_x, ontdek_y, &ontdek_url);
        }
    }

    // ── Badges ───────────────────────────────────────────────────
    let badges = brand.page_badges("cover");
    if !badges.is_empty() {
        let badge_radius = sf(
            brand.page_f64("cover", "badge_radius_ref", 17.0),
            page_h.0,
        );
        let badge_font_size = sf(
            brand.page_f64("cover", "badge_font_size_ref", 10.2),
            page_h.0,
        );

        for badge in &badges {
            let bx = sx(badge.x_ref, page_w.0);
            let by = sy(badge.y_ref, page_h.0);
            let bw = sx(badge.w_ref, page_w.0);
            let bh = sy(badge.h_ref, page_h.0);

            // Badge background (rounded rect)
            if let Some(bg) = Color::from_hex(&badge.bg_color) {
                dl.set_fill_color(bg);
                dl.draw_rounded_rect(bx, by, bw, bh, badge_radius, true, false);
            }

            // Badge text (centered)
            if let Some(tc) = Color::from_hex(&badge.text_color) {
                dl.set_fill_color(tc);
            }
            dl.set_font(&heading_font, badge_font_size);
            dl.draw_text_center(
                Pt(bx.0 + bw.0 / 2.0),
                Pt(by.0 + bh.0 / 2.0 - badge_font_size.0 / 3.0),
                &badge.label,
            );
        }
    }

    // ── Project metadata ─────────────────────────────────────────
    let meta_x = sx(54.28, page_w.0);
    let mut meta_y = sy(120.0, page_h.0);
    let meta_leading = Pt(14.0);
    dl.set_fill_color(Color::WHITE);
    dl.set_font(&body_font, Pt(9.0));

    if let Some(ref pn) = data.project_number {
        dl.draw_text(meta_x, meta_y, &format!("Projectnummer: {}", pn));
        meta_y = Pt(meta_y.0 + meta_leading.0);
    }
    if let Some(ref client) = data.client {
        dl.draw_text(meta_x, meta_y, &format!("Opdrachtgever: {}", client));
        meta_y = Pt(meta_y.0 + meta_leading.0);
    }
    if let Some(ref date) = data.date {
        dl.draw_text(meta_x, meta_y, &format!("Datum: {}", date));
    }

    RawPage {
        page_size: A4,
        draw_list: dl,
    }
}

// ── Colofon page ─────────────────────────────────────────────────────

/// Build a colofon page following brand.yaml `pages.colofon` spec.
///
/// Uses precise y_pt positions from the brand config for field placement,
/// separator lines, footer rect, logo, and page number.
pub fn build_colofon_page(data: &ReportData, brand: &BrandConfig) -> RawPage {
    let mut dl = DrawList::new();
    let primary = brand_primary(brand);
    let secondary = brand_secondary(brand);
    let page_h = A4.height;

    let heading_font = resolve_font(brand, "heading");
    let body_font = resolve_font(brand, "body");

    // ── Report type heading ──────────────────────────────────────
    let rt_font = brand.page_str("colofon", "report_type_font", &heading_font);
    let rt_size = brand.page_f64("colofon", "report_type_size", 22.0);
    let rt_color = brand
        .page_str("colofon", "report_type_color", "#40124A");
    let rt_x = Pt(brand.page_f64("colofon", "report_type_x_pt", 70.9) as f32);
    let rt_y = Pt(brand.page_f64("colofon", "report_type_y_pt", 57.3) as f32);

    dl.set_fill_color(
        Color::from_hex(&rt_color).unwrap_or(primary),
    );
    dl.set_font(&rt_font, Pt(rt_size as f32));
    // Report type text = template name (capitalized)
    let report_type_text = capitalize_first(&data.template);
    dl.draw_text(rt_x, rt_y, &report_type_text);

    // ── Subtitle ─────────────────────────────────────────────────
    let st_font = brand.page_str("colofon", "subtitle_font", &body_font);
    let st_size = brand.page_f64("colofon", "subtitle_size", 14.0);
    let st_color = brand.page_str("colofon", "subtitle_color", "#38BDA0");
    let st_x = Pt(brand.page_f64("colofon", "subtitle_x_pt", 70.9) as f32);
    let st_y = Pt(brand.page_f64("colofon", "subtitle_y_pt", 86.8) as f32);

    dl.set_fill_color(Color::from_hex(&st_color).unwrap_or(secondary));
    dl.set_font(&st_font, Pt(st_size as f32));
    // Subtitle = project name
    dl.draw_text(st_x, st_y, &data.project);

    // ── Field rendering ──────────────────────────────────────────
    let label_x = Pt(brand.page_f64("colofon", "label_x_pt", 103.0) as f32);
    let value_x = Pt(brand.page_f64("colofon", "value_x_pt", 229.0) as f32);
    let first_labels_color = Color::from_hex(
        &brand.page_str("colofon", "first_labels_color", "#40124A"),
    )
    .unwrap_or(primary);
    let other_labels_color = Color::from_hex(
        &brand.page_str("colofon", "other_labels_color", "#38BDA0"),
    )
    .unwrap_or(secondary);
    let value_color = Color::from_hex(
        &brand.page_str("colofon", "value_color", "#40124A"),
    )
    .unwrap_or(primary);

    let label_font_name = brand.page_str("colofon", "label_font", &heading_font);
    let label_size = Pt(brand.page_f64("colofon", "label_size", 10.0) as f32);
    let value_font_name = brand.page_str("colofon", "value_font", &body_font);
    let value_size = Pt(brand.page_f64("colofon", "value_size", 10.0) as f32);

    // Line config
    let line_x1 = Pt(brand.page_f64("colofon", "line_x1_pt", 102.0) as f32);
    let line_x2 = Pt(brand.page_f64("colofon", "line_x2_pt", 420.0) as f32);
    let line_stroke = Pt(brand.page_f64("colofon", "line_stroke_pt", 0.25) as f32);
    let line_color = Color::from_hex(
        &brand.page_str("colofon", "line_color", "#40124A"),
    )
    .unwrap_or(primary);

    let fields = brand.page_fields("colofon");
    let mut label_count = 0u32;

    for field in &fields {
        let y = Pt(field.y_pt as f32);

        if field.field_type == "line" {
            // Separator line
            dl.set_stroke_color(line_color);
            dl.set_line_width(line_stroke);
            dl.draw_line(line_x1, y, line_x2, y);
            continue;
        }

        // Resolve field value from report data
        let value = resolve_colofon_field(data, &field.field_type);
        if value.is_empty() && field.label.is_empty() {
            continue; // Skip empty sub-fields
        }

        // Label
        if !field.label.is_empty() {
            label_count += 1;
            let lbl_color = if label_count <= 2 {
                first_labels_color
            } else {
                other_labels_color
            };
            dl.set_fill_color(lbl_color);
            dl.set_font(&label_font_name, label_size);
            dl.draw_text(label_x, y, &field.label);
        }

        // Value
        if !value.is_empty() {
            dl.set_fill_color(value_color);
            dl.set_font(&value_font_name, value_size);
            dl.draw_text(value_x, y, &value);
        }
    }

    // ── Revision history ─────────────────────────────────────────
    if let Some(ref colofon) = data.colofon {
        if !colofon.revision_history.is_empty() {
            // Position revision table below the fields area
            let rev_start_y = Pt(680.0);
            let col_version_x = label_x;
            let col_date_x = Pt(label_x.0 + 60.0);
            let col_author_x = Pt(label_x.0 + 120.0);
            let col_desc_x = Pt(label_x.0 + 190.0);

            dl.set_fill_color(other_labels_color);
            dl.set_font(&label_font_name, Pt(8.0));
            dl.draw_text(col_version_x, rev_start_y, "Versie");
            dl.draw_text(col_date_x, rev_start_y, "Datum");
            dl.draw_text(col_author_x, rev_start_y, "Auteur");
            dl.draw_text(col_desc_x, rev_start_y, "Omschrijving");

            let mut rev_y = Pt(rev_start_y.0 + 14.0);
            dl.set_fill_color(value_color);
            dl.set_font(&value_font_name, Pt(8.0));
            for rev in &colofon.revision_history {
                dl.draw_text(col_version_x, rev_y, &rev.version);
                dl.draw_text(col_date_x, rev_y, &rev.date);
                dl.draw_text(
                    col_author_x,
                    rev_y,
                    rev.author.as_deref().unwrap_or("-"),
                );
                dl.draw_text(col_desc_x, rev_y, &rev.description);
                rev_y = Pt(rev_y.0 + 13.0);
            }
        }

        // ── Disclaimer ───────────────────────────────────────────
        if let Some(ref disclaimer) = colofon.disclaimer {
            let disclaimer_y = Pt(750.0);
            dl.set_fill_color(Color::GREY);
            let italic_font = brand.resolve_font_name("italic");
            dl.set_font(&italic_font, Pt(7.0));
            dl.draw_text(label_x, disclaimer_y, disclaimer);
        }
    }

    // ── Footer rect (turquoise block) ────────────────────────────
    if let Some(fr) = brand.page_rect("colofon", "footer_rect") {
        let fr_color = Color::from_hex(
            &brand.page_str("colofon", "footer_rect_color", "#38BDA0"),
        )
        .unwrap_or(secondary);
        dl.set_fill_color(fr_color);
        dl.draw_rect(
            Pt(fr[0] as f32),
            Pt(fr[1] as f32),
            Pt((fr[2] - fr[0]) as f32), // width = x2 - x1
            Pt((fr[3] - fr[1]) as f32), // height = y2 - y1
            true,
            false,
        );
    }

    // ── Logo in footer ───────────────────────────────────────────
    if let Some(logo_path) = brand.logo_path("tagline").or_else(|| brand.logo_path("main")) {
        if let Ok(logo_data) = std::fs::read(&logo_path) {
            // Place logo in the footer area (bottom-left)
            dl.draw_image(logo_data, Pt(15.0), Pt(page_h.0 - 55.0), Pt(80.0), Pt(30.0));
        }
    }

    // ── Page number ──────────────────────────────────────────────
    let pn_x = Pt(brand.page_f64("colofon", "page_num_x_pt", 534.0) as f32);
    let pn_y = Pt(brand.page_f64("colofon", "page_num_y_pt", 796.3) as f32);
    let pn_font = brand.page_str("colofon", "page_num_font", &body_font);
    let pn_size = Pt(brand.page_f64("colofon", "page_num_size", 9.5) as f32);
    let pn_color = Color::from_hex(
        &brand.page_str("colofon", "page_num_color", "#38BDA0"),
    )
    .unwrap_or(secondary);

    dl.set_fill_color(pn_color);
    dl.set_font(&pn_font, pn_size);
    dl.draw_text(pn_x, pn_y, "2"); // Colofon is typically page 2

    RawPage {
        page_size: A4,
        draw_list: dl,
    }
}

/// Resolve a colofon field type to its value from report data.
fn resolve_colofon_field(data: &ReportData, field_type: &str) -> String {
    let colofon = data.colofon.as_ref();
    match field_type {
        "project" => data.project.clone(),
        "client" => data.client.clone().unwrap_or_default(),
        "opdrachtgever_naam" => colofon
            .and_then(|c| c.opdrachtgever_naam.clone())
            .unwrap_or_default(),
        "opdrachtgever_adres" => colofon
            .and_then(|c| c.opdrachtgever_adres.clone())
            .unwrap_or_default(),
        "adviseur_bedrijf" => colofon
            .and_then(|c| c.adviseur_bedrijf.clone())
            .unwrap_or_default(),
        "adviseur_naam" => colofon
            .and_then(|c| c.adviseur_naam.clone())
            .unwrap_or_default(),
        "norms" => colofon
            .and_then(|c| c.normen.clone())
            .unwrap_or_default(),
        "document_description" => colofon
            .and_then(|c| c.documentgegevens.clone())
            .unwrap_or_default(),
        "date" => colofon
            .and_then(|c| c.datum.clone())
            .or_else(|| data.date.clone())
            .unwrap_or_default(),
        "phase" => colofon
            .and_then(|c| c.fase.clone())
            .unwrap_or_default(),
        "status" => colofon
            .and_then(|c| c.status_colofon.clone())
            .unwrap_or_else(|| format!("{:?}", data.status)),
        "document_code" => colofon
            .and_then(|c| c.kenmerk.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Capitalize the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

// ── TOC page ─────────────────────────────────────────────────────────

/// Build a table of contents page following brand.yaml `pages.toc` spec.
///
/// Uses 3-column layout (number, title, page) with distinct level1/level2
/// fonts, colors, and spacing from the brand config.
pub fn build_toc_page(data: &ReportData, brand: &BrandConfig) -> RawPage {
    let mut dl = DrawList::new();
    let text_color = brand_text(brand);

    let body_font = resolve_font(brand, "body");

    // ── Title ────────────────────────────────────────────────────
    let toc_title = data
        .toc
        .as_ref()
        .map(|t| t.title.as_str())
        .unwrap_or("Inhoudsopgave");

    let title_font = brand.page_str("toc", "title_font", &body_font);
    let title_size = Pt(brand.page_f64("toc", "title_size", 18.0) as f32);
    let title_color = Color::from_hex(
        &brand.page_str("toc", "title_color", "#45243D"),
    )
    .unwrap_or(text_color);
    let title_x = Pt(brand.page_f64("toc", "title_x_pt", 90.0) as f32);
    let title_y = Pt(brand.page_f64("toc", "title_y_pt", 74.9) as f32);

    dl.set_fill_color(title_color);
    dl.set_font(&title_font, title_size);
    dl.draw_text(title_x, title_y, toc_title);

    // ── Level styling ────────────────────────────────────────────
    let l1_font = brand.page_str("toc", "level1_font", &body_font);
    let l1_size = Pt(brand.page_f64("toc", "level1_size", 12.0) as f32);
    let l1_color = Color::from_hex(
        &brand.page_str("toc", "level1_color", "#56B49B"),
    )
    .unwrap_or(text_color);

    let l2_font = brand.page_str("toc", "level2_font", &body_font);
    let l2_size = Pt(brand.page_f64("toc", "level2_size", 9.5) as f32);
    let l2_color = Color::from_hex(
        &brand.page_str("toc", "level2_color", "#45243D"),
    )
    .unwrap_or(text_color);

    // ── Column positions ─────────────────────────────────────────
    let number_x = Pt(brand.page_f64("toc", "number_x_pt", 90.0) as f32);
    let text_x = Pt(brand.page_f64("toc", "text_x_pt", 160.9) as f32);
    let page_x = Pt(brand.page_f64("toc", "page_x_pt", 515.4) as f32);

    // ── Spacing ──────────────────────────────────────────────────
    let chapter_spacing = Pt(brand.page_f64("toc", "chapter_spacing_pt", 39.0) as f32);
    let item_spacing = Pt(brand.page_f64("toc", "item_spacing_pt", 17.0) as f32);

    let max_depth = data.toc.as_ref().map(|t| t.max_depth).unwrap_or(3);

    // ── Entries ──────────────────────────────────────────────────
    let mut cursor_y = Pt(title_y.0 + title_size.0 + chapter_spacing.0);
    let mut chapter_num = 0u32;
    let mut sub_num = 0u32;

    for section in &data.sections {
        if section.level > max_depth {
            continue;
        }

        if section.level == 1 {
            chapter_num += 1;
            sub_num = 0;

            // Extra spacing before chapter (except first)
            if chapter_num > 1 {
                cursor_y = Pt(cursor_y.0 + chapter_spacing.0);
            }

            let number_text = format!("{}", chapter_num);

            dl.set_fill_color(l1_color);
            dl.set_font(&l1_font, l1_size);
            dl.draw_text(number_x, cursor_y, &number_text);
            dl.draw_text(text_x, cursor_y, &section.title);
            // Page placeholder
            dl.draw_text_right(page_x, cursor_y, "\u{2014}");

            cursor_y = Pt(cursor_y.0 + item_spacing.0);
        } else {
            sub_num += 1;
            let number_text = format!("{}.{}", chapter_num, sub_num);

            dl.set_fill_color(l2_color);
            dl.set_font(&l2_font, l2_size);
            dl.draw_text(number_x, cursor_y, &number_text);
            dl.draw_text(text_x, cursor_y, &section.title);
            dl.draw_text_right(page_x, cursor_y, "\u{2014}");

            cursor_y = Pt(cursor_y.0 + item_spacing.0);
        }
    }

    RawPage {
        page_size: A4,
        draw_list: dl,
    }
}

// ── Backcover page ───────────────────────────────────────────────────

/// Build a backcover page following brand.yaml `pages.backcover` spec.
///
/// Uses polygons for white/purple shapes when configured,
/// falling back to a simplified layout.
pub fn build_backcover_page(data: &ReportData, brand: &BrandConfig) -> RawPage {
    let _ = data;
    let mut dl = DrawList::new();
    let primary = brand_primary(brand);
    let secondary = brand_secondary(brand);

    let page_w = A4.width;
    let page_h = A4.height;

    let heading_font = resolve_font(brand, "heading");
    let body_font = resolve_font(brand, "body");

    // ── Full-page secondary background ───────────────────────────
    dl.set_fill_color(secondary);
    dl.draw_rect(Pt(0.0), Pt(0.0), page_w, page_h, true, false);

    // ── White polygon ────────────────────────────────────────────
    let white_poly = brand.page_polygon("backcover", "white_polygon");
    if !white_poly.is_empty() {
        dl.set_fill_color(Color::WHITE);
        let points: Vec<(Pt, Pt)> = white_poly
            .iter()
            .map(|(x, y)| (sx(*x, page_w.0), sy(*y, page_h.0)))
            .collect();
        dl.draw_polygon(points, true, false);
    }

    // ── Purple triangle ──────────────────────────────────────────
    let purple_tri = brand.page_polygon("backcover", "purple_triangle");
    if !purple_tri.is_empty() {
        dl.set_fill_color(primary);
        let points: Vec<(Pt, Pt)> = purple_tri
            .iter()
            .map(|(x, y)| (sx(*x, page_w.0), sy(*y, page_h.0)))
            .collect();
        dl.draw_polygon(points, true, false);
    }

    // ── Logo ─────────────────────────────────────────────────────
    // Set fill to white (logo sits on white polygon, not purple triangle).
    // This ensures correct alpha compositing background.
    dl.set_fill_color(Color::WHITE);
    let logo_key = brand.page_str("backcover", "logo_key", "main");
    if let Some(logo_path) = brand.logo_path(&logo_key) {
        if let Ok(logo_data) = std::fs::read(&logo_path) {
            let logo_x = sx(
                brand.page_f64("backcover", "logo_x_ref", 268.0),
                page_w.0,
            );
            let logo_y = sy(
                brand.page_f64("backcover", "logo_y_ref", 337.0),
                page_h.0,
            );
            let logo_w = sx(
                brand.page_f64("backcover", "logo_w_ref", 170.0),
                page_w.0,
            );
            let logo_h = Pt(logo_w.0 * 0.35);
            dl.draw_image(logo_data, logo_x, logo_y, logo_w, logo_h);
        }
    }

    // ── Contact info ─────────────────────────────────────────────
    let contact_x = sx(
        brand.page_f64("backcover", "contact_x_ref", 268.0),
        page_w.0,
    );
    let mut contact_y = sy(
        brand.page_f64("backcover", "contact_y_ref", 185.0),
        page_h.0,
    );
    let contact_line_h = sy(
        brand.page_f64("backcover", "contact_line_h_ref", 20.0),
        page_h.0,
    );
    let name_size = sf(
        brand.page_f64("backcover", "contact_name_size_ref", 11.0),
        page_h.0,
    );
    let detail_size = sf(
        brand.page_f64("backcover", "contact_detail_size_ref", 9.0),
        page_h.0,
    );

    dl.set_fill_color(primary);

    // "Ontdek ons" prefix
    let ontdek_prefix = brand.page_str("backcover", "ontdek_prefix", "");
    if !ontdek_prefix.is_empty() {
        dl.set_font(&body_font, detail_size);
        let website = brand.contact.get("website").cloned().unwrap_or_default();
        dl.draw_text(contact_x, contact_y, &format!("{}{}", ontdek_prefix, website));
        contact_y = Pt(contact_y.0 + contact_line_h.0 * 2.0);
    }

    // Company name
    if let Some(name) = brand.contact.get("name") {
        dl.set_font(&heading_font, name_size);
        dl.draw_text(contact_x, contact_y, name);
        contact_y = Pt(contact_y.0 + contact_line_h.0);
    }

    // Address
    if let Some(address) = brand.contact.get("address") {
        dl.set_font(&body_font, detail_size);
        dl.draw_text(contact_x, contact_y, address);
    }

    RawPage {
        page_size: A4,
        draw_list: dl,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::{BrandIdentity, ZoneConfig};
    use crate::schema::*;
    use std::collections::HashMap;

    fn test_brand() -> BrandConfig {
        let mut colors = HashMap::new();
        colors.insert("primary".to_string(), "#40124A".to_string());
        colors.insert("secondary".to_string(), "#38BDA0".to_string());
        colors.insert("text".to_string(), "#45243D".to_string());

        let mut contact = HashMap::new();
        contact.insert("name".to_string(), "3BM Bouwkunde".to_string());
        contact.insert("address".to_string(), "Street 1 | City".to_string());
        contact.insert("website".to_string(), "www.3bm.nl".to_string());

        BrandConfig {
            brand: BrandIdentity {
                name: "3BM Bouwkunde".to_string(),
                slug: "3bm".to_string(),
                tenant: None,
                tagline: Some("Engineering excellence".to_string()),
            },
            colors,
            fonts: HashMap::new(),
            logos: HashMap::new(),
            contact,
            header: ZoneConfig::default(),
            footer: ZoneConfig::default(),
            styles: HashMap::new(),
            pages: HashMap::new(),
            stationery: HashMap::new(),
            modules: HashMap::new(),
            module_config: HashMap::new(),
            tenant_modules: serde_yaml::Value::Null,
            font_files: HashMap::new(),
            brand_dir: None,
        }
    }

    fn minimal_data() -> ReportData {
        ReportData {
            template: "structural".to_string(),
            project: "Test Project".to_string(),
            tenant: None,
            format: PaperFormat::A4,
            orientation: Orientation::Portrait,
            project_number: Some("2026-001".to_string()),
            client: Some("Test Client".to_string()),
            author: "JK".to_string(),
            date: Some("2026-03-15".to_string()),
            version: "1.0".to_string(),
            status: ReportStatus::Concept,
            cover: Some(Cover {
                subtitle: Some("Constructieve berekening".to_string()),
                image: None,
                extra_fields: HashMap::new(),
            }),
            colofon: Some(Colofon {
                enabled: true,
                opdrachtgever_naam: Some("Gemeente Den Haag".to_string()),
                opdrachtgever_contact: None,
                opdrachtgever_adres: None,
                adviseur_bedrijf: Some("3BM Bouwkunde".to_string()),
                adviseur_naam: Some("J. Kragten".to_string()),
                normen: None,
                documentgegevens: None,
                datum: Some("2026-03-15".to_string()),
                fase: None,
                status_colofon: Some("Concept".to_string()),
                kenmerk: Some("2026-001-R01".to_string()),
                adviseur_email: None,
                adviseur_telefoon: None,
                adviseur_functie: None,
                adviseur_registratie: None,
                extra_fields: HashMap::new(),
                revision_history: vec![RevisionEntry {
                    version: "1.0".to_string(),
                    date: "2026-03-15".to_string(),
                    author: Some("JK".to_string()),
                    description: "Eerste uitgave".to_string(),
                }],
                disclaimer: Some("Dit rapport is vertrouwelijk.".to_string()),
            }),
            toc: Some(TocConfig {
                enabled: true,
                title: "Inhoudsopgave".to_string(),
                max_depth: 3,
            }),
            sections: vec![
                Section {
                    title: "Uitgangspunten".to_string(),
                    level: 1,
                    content: vec![],
                    orientation: None,
                    page_break_before: false,
                },
                Section {
                    title: "Belastingen".to_string(),
                    level: 1,
                    content: vec![],
                    orientation: None,
                    page_break_before: false,
                },
                Section {
                    title: "Permanente belasting".to_string(),
                    level: 2,
                    content: vec![],
                    orientation: None,
                    page_break_before: false,
                },
            ],
            backcover: Some(BackcoverConfig { enabled: true }),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_brand_color_helpers() {
        let brand = test_brand();
        assert_eq!(brand_primary(&brand), Color::rgb(0x40, 0x12, 0x4A));
        assert_eq!(brand_secondary(&brand), Color::rgb(0x38, 0xBD, 0xA0));
        assert_eq!(brand_text(&brand), Color::rgb(0x45, 0x24, 0x3D));
    }

    #[test]
    fn test_brand_color_fallback() {
        let brand = BrandConfig {
            brand: BrandIdentity::default(),
            colors: HashMap::new(),
            fonts: HashMap::new(),
            logos: HashMap::new(),
            contact: HashMap::new(),
            header: ZoneConfig::default(),
            footer: ZoneConfig::default(),
            styles: HashMap::new(),
            pages: HashMap::new(),
            stationery: HashMap::new(),
            modules: HashMap::new(),
            module_config: HashMap::new(),
            tenant_modules: serde_yaml::Value::Null,
            font_files: HashMap::new(),
            brand_dir: None,
        };
        // Should fall back to 3BM defaults
        assert_eq!(brand_primary(&brand), Color::rgb(0x40, 0x12, 0x4A));
        assert_eq!(brand_secondary(&brand), Color::rgb(0x38, 0xBD, 0xA0));
        assert_eq!(brand_text(&brand), Color::rgb(0x45, 0x24, 0x3D));
    }

    #[test]
    fn test_build_cover_page() {
        let data = minimal_data();
        let brand = test_brand();
        let page = build_cover_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // Should have draw ops: rect + font + text (brand name) + font + text (title) + ...
        assert!(
            page.draw_list.ops.len() >= 5,
            "Cover page should have at least 5 draw ops, got {}",
            page.draw_list.ops.len()
        );
    }

    #[test]
    fn test_build_cover_page_minimal() {
        let mut data = minimal_data();
        data.cover = None;
        data.project_number = None;
        data.client = None;
        data.date = None;

        let brand = test_brand();
        let page = build_cover_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        assert!(!page.draw_list.ops.is_empty());
    }

    #[test]
    fn test_build_colofon_page() {
        let data = minimal_data();
        let brand = test_brand();
        let page = build_colofon_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // Should have many draw ops: title + rows + separators + revision history + disclaimer
        assert!(
            page.draw_list.ops.len() >= 10,
            "Colofon page should have at least 10 draw ops, got {}",
            page.draw_list.ops.len()
        );
    }

    #[test]
    fn test_build_colofon_page_without_colofon() {
        let mut data = minimal_data();
        data.colofon = None;

        let brand = test_brand();
        let page = build_colofon_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        assert!(!page.draw_list.ops.is_empty());
    }

    #[test]
    fn test_build_toc_page() {
        let data = minimal_data();
        let brand = test_brand();
        let page = build_toc_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // Title + 3 sections (each has font + text ops)
        assert!(
            page.draw_list.ops.len() >= 6,
            "TOC page should have at least 6 draw ops, got {}",
            page.draw_list.ops.len()
        );
    }

    #[test]
    fn test_build_toc_page_empty_sections() {
        let mut data = minimal_data();
        data.sections.clear();

        let brand = test_brand();
        let page = build_toc_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // At minimum: title font + text
        assert!(page.draw_list.ops.len() >= 2);
    }

    #[test]
    fn test_build_toc_respects_max_depth() {
        let mut data = minimal_data();
        data.toc = Some(TocConfig {
            enabled: true,
            title: "Contents".to_string(),
            max_depth: 1,
        });
        // Add a level-2 section that should be filtered out
        data.sections.push(Section {
            title: "Should be filtered".to_string(),
            level: 2,
            content: vec![],
            orientation: None,
            page_break_before: false,
        });

        let brand = test_brand();
        let page_depth_1 = build_toc_page(&data, &brand);

        // With max_depth=3
        data.toc = Some(TocConfig {
            enabled: true,
            title: "Contents".to_string(),
            max_depth: 3,
        });
        let page_depth_3 = build_toc_page(&data, &brand);

        // depth_3 should have more ops (the level-2 entries are included)
        assert!(page_depth_3.draw_list.ops.len() > page_depth_1.draw_list.ops.len());
    }

    #[test]
    fn test_build_backcover_page() {
        let data = minimal_data();
        let brand = test_brand();
        let page = build_backcover_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // Background rect + accent rect + brand name + tagline + contact lines
        assert!(
            page.draw_list.ops.len() >= 6,
            "Backcover should have at least 6 draw ops, got {}",
            page.draw_list.ops.len()
        );
    }

    #[test]
    fn test_build_backcover_no_contact() {
        let data = minimal_data();
        let mut brand = test_brand();
        brand.contact.clear();
        brand.brand.tagline = None;

        let page = build_backcover_page(&data, &brand);

        assert_eq!(page.page_size, A4);
        // Should still render background rect at minimum
        assert!(
            !page.draw_list.ops.is_empty(),
            "Backcover should have at least 1 draw op (background)"
        );
    }
}
