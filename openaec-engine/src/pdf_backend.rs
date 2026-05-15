//! Low-level PDF generation via pdf-writer.
//!
//! Wraps pdf-writer to provide a simple drawing API:
//! text, lines, rectangles, images, and stationery PDF embedding.

use std::sync::Arc;

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo};

use crate::error::{EngineError, Result};

/// Conversion: mm → points (1pt = 1/72 inch).
pub const MM_TO_PT: f64 = 2.834_645_669_3;

/// RGB color (0.0–1.0).
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self { r: 0.0, g: 0.0, b: 0.0 }
    }
}

/// Handle to a registered font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub usize);

/// Handle to a page (index into pages vec).
#[derive(Debug, Clone, Copy)]
pub struct PageHandle(pub usize);

/// Image format.
#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

/// Represents a single drawing operation on a page.
#[derive(Debug, Clone)]
pub(crate) enum DrawOp {
    Text {
        text: String,
        x: f32,
        y: f32,
        font_id: FontId,
        size: f32,
        color: Color,
    },
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        color: Color,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<Color>,
        stroke: Option<(f32, Color)>,
    },
    Image {
        image_idx: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    StationeryPage {
        stationery_idx: usize,
    },
}

/// A page in the document.
struct PageData {
    width_pt: f32,
    height_pt: f32,
    ops: Vec<DrawOp>,
}

/// Registered font data.
pub(crate) struct RegisteredFont {
    pub data: Arc<Vec<u8>>,
    pub name: String,
}

/// Registered image data.
struct RegisteredImage {
    data: Vec<u8>,
    format: ImageFormat,
    width: u32,
    height: u32,
}

/// Registered stationery PDF.
struct RegisteredStationery {
    data: Vec<u8>,
    page_index: usize,
}

/// PDF backend — accumulates drawing operations, then serializes to PDF.
pub struct PdfBackend {
    pages: Vec<PageData>,
    fonts: Vec<RegisteredFont>,
    images: Vec<RegisteredImage>,
    stationery: Vec<RegisteredStationery>,
}

impl PdfBackend {
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            fonts: Vec::new(),
            images: Vec::new(),
            stationery: Vec::new(),
        }
    }

    /// Add a new page with given dimensions in mm.
    pub fn add_page(&mut self, width_mm: f64, height_mm: f64) -> PageHandle {
        let idx = self.pages.len();
        self.pages.push(PageData {
            width_pt: (width_mm * MM_TO_PT) as f32,
            height_pt: (height_mm * MM_TO_PT) as f32,
            ops: Vec::new(),
        });
        PageHandle(idx)
    }

    /// Register a TrueType font. Returns a FontId for drawing.
    pub fn register_font(&mut self, name: String, data: Vec<u8>) -> FontId {
        let id = FontId(self.fonts.len());
        self.fonts.push(RegisteredFont {
            data: Arc::new(data),
            name,
        });
        id
    }

    /// Draw text at exact position (in points, PDF coordinate system).
    pub fn draw_text(
        &mut self,
        page: PageHandle,
        text: &str,
        x: f32,
        y: f32,
        font_id: FontId,
        size: f32,
        color: Color,
    ) {
        self.pages[page.0].ops.push(DrawOp::Text {
            text: text.to_string(),
            x, y, font_id, size, color,
        });
    }

    /// Draw a line between two points.
    pub fn draw_line(
        &mut self,
        page: PageHandle,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        color: Color,
    ) {
        self.pages[page.0].ops.push(DrawOp::Line {
            x0, y0, x1, y1, width, color,
        });
    }

    /// Draw a rectangle.
    pub fn draw_rect(
        &mut self,
        page: PageHandle,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<Color>,
        stroke: Option<(f32, Color)>,
    ) {
        self.pages[page.0].ops.push(DrawOp::Rect {
            x, y, w, h, fill, stroke,
        });
    }

    /// Register an image. Returns index for drawing.
    pub fn register_image(
        &mut self,
        data: Vec<u8>,
        format: ImageFormat,
        width: u32,
        height: u32,
    ) -> usize {
        let idx = self.images.len();
        self.images.push(RegisteredImage { data, format, width, height });
        idx
    }

    /// Draw a previously registered image at position with size (in points).
    pub fn draw_image(
        &mut self,
        page: PageHandle,
        image_idx: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        self.pages[page.0].ops.push(DrawOp::Image {
            image_idx, x, y, w, h,
        });
    }

    /// Register stationery PDF bytes + page index.
    pub fn register_stationery(&mut self, data: Vec<u8>, page_index: usize) -> usize {
        let idx = self.stationery.len();
        self.stationery.push(RegisteredStationery { data, page_index });
        idx
    }

    /// Embed stationery as page background.
    pub fn embed_stationery(&mut self, page: PageHandle, stationery_idx: usize) {
        self.pages[page.0].ops.insert(0, DrawOp::StationeryPage { stationery_idx });
    }

    /// Number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Serialize all accumulated pages/operations to PDF bytes.
    pub fn finish(self) -> Result<Vec<u8>> {
        let mut pdf = Pdf::new();
        let mut next_id = 1;
        let mut alloc = || { let r = Ref::new(next_id); next_id += 1; r };

        let catalog_ref = alloc();
        let page_tree_ref = alloc();

        // ── Allocate refs for fonts (Type0 + CIDFont + descriptor + stream + cmap) ──
        let mut font_refs = Vec::new();
        let mut descendant_refs = Vec::new();
        let mut descriptor_refs = Vec::new();
        let mut stream_refs = Vec::new();
        let mut cmap_refs = Vec::new();
        for _ in &self.fonts {
            font_refs.push(alloc());
            descendant_refs.push(alloc());
            descriptor_refs.push(alloc());
            stream_refs.push(alloc());
            cmap_refs.push(alloc());
        }

        // ── Allocate refs for images ──
        let image_refs: Vec<Ref> = self.images.iter().map(|_| alloc()).collect();

        // ── Allocate refs for stationery FormXObjects ──
        let stationery_refs: Vec<Ref> = self.stationery.iter().map(|_| alloc()).collect();

        // ── Allocate refs for pages + content streams ──
        let page_refs: Vec<Ref> = self.pages.iter().map(|_| alloc()).collect();
        let content_refs: Vec<Ref> = self.pages.iter().map(|_| alloc()).collect();

        // ── Write fonts ──
        let font_names: Vec<String> = (0..self.fonts.len()).map(|i| format!("F{}", i)).collect();

        for (i, font) in self.fonts.iter().enumerate() {
            let face = ttf_parser::Face::parse(&font.data, 0)
                .map_err(|e| EngineError::Font(format!("Parse '{}': {}", font.name, e)))?;

            let units_per_em = face.units_per_em() as f32;
            let scale = 1000.0 / units_per_em;

            let bbox = face.global_bounding_box();
            let font_bbox = Rect::new(
                bbox.x_min as f32 * scale,
                bbox.y_min as f32 * scale,
                bbox.x_max as f32 * scale,
                bbox.y_max as f32 * scale,
            );

            // Font stream — embed full TTF
            pdf.stream(stream_refs[i], &font.data)
                .pair(Name(b"Subtype"), Name(b"OpenType"));

            // Font descriptor
            let mut desc = pdf.font_descriptor(descriptor_refs[i]);
            desc.name(Name(font.name.as_bytes()));
            desc.flags(FontFlags::NON_SYMBOLIC);
            desc.bbox(font_bbox);
            desc.ascent(face.ascender() as f32 * scale);
            desc.descent(face.descender() as f32 * scale);
            desc.font_file3(stream_refs[i]);
            desc.finish();

            // CIDFont (descendant)
            let num_glyphs = face.number_of_glyphs();
            let mut cid = pdf.cid_font(descendant_refs[i]);
            cid.subtype(CidFontType::Type2);
            cid.base_font(Name(font.name.as_bytes()));
            cid.system_info(SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            });
            cid.font_descriptor(descriptor_refs[i]);
            cid.default_width(1000.0);

            // Glyph widths
            let mut widths_vec: Vec<f32> = Vec::new();
            for gid in 0..num_glyphs {
                let w = face
                    .glyph_hor_advance(ttf_parser::GlyphId(gid))
                    .map(|a| a as f32 * scale)
                    .unwrap_or(0.0);
                widths_vec.push(w);
            }
            if !widths_vec.is_empty() {
                let mut w = cid.widths();
                w.consecutive(0, widths_vec.iter().copied());
            }
            cid.finish();

            // ToUnicode CMap
            let cmap_data = build_identity_cmap(num_glyphs);
            pdf.stream(cmap_refs[i], cmap_data.as_bytes());

            // Type0 font
            let mut t0 = pdf.type0_font(font_refs[i]);
            t0.base_font(Name(font.name.as_bytes()));
            t0.encoding_predefined(Name(b"Identity-H"));
            t0.descendant_font(descendant_refs[i]);
            t0.to_unicode(cmap_refs[i]);
            t0.finish();
        }

        // ── Write images ──
        for (i, img) in self.images.iter().enumerate() {
            match img.format {
                ImageFormat::Jpeg => {
                    let mut xobj = pdf.image_xobject(image_refs[i], &img.data);
                    xobj.width(img.width as i32);
                    xobj.height(img.height as i32);
                    xobj.color_space().device_rgb();
                    xobj.bits_per_component(8);
                    xobj.filter(pdf_writer::Filter::DctDecode);
                }
                ImageFormat::Png => {
                    let decoded = image::load_from_memory(&img.data)
                        .map_err(|e| EngineError::Image(format!("PNG decode: {}", e)))?;
                    let rgb = decoded.to_rgb8();
                    let raw = rgb.as_raw();

                    let mut xobj = pdf.image_xobject(image_refs[i], raw);
                    xobj.width(img.width as i32);
                    xobj.height(img.height as i32);
                    xobj.color_space().device_rgb();
                    xobj.bits_per_component(8);
                }
            }
        }

        // ── Write stationery as FormXObjects ──
        // For now, stationery embedding is a placeholder.
        // Full PDF-in-PDF embedding requires lopdf to extract page content.
        for (i, _stat) in self.stationery.iter().enumerate() {
            // Create a minimal empty form XObject as placeholder
            let empty_content = Content::new().finish();
            let mut form = pdf.form_xobject(stationery_refs[i], &empty_content);
            form.bbox(Rect::new(0.0, 0.0, 595.28, 841.89)); // A4
            form.finish();
        }

        // ── Write pages ──
        for (pi, page) in self.pages.iter().enumerate() {
            // Build content stream
            let mut content = Content::new();

            for op in &page.ops {
                match op {
                    DrawOp::Text { text, x, y, font_id, size, color } => {
                        let fname = &font_names[font_id.0];
                        let encoded = encode_text_to_gids(text, &self.fonts[font_id.0]);

                        content.save_state();
                        content.set_fill_rgb(color.r, color.g, color.b);
                        content.begin_text();
                        content.set_font(Name(fname.as_bytes()), *size);
                        content.next_line(*x, *y);
                        content.show(Str(&encoded));
                        content.end_text();
                        content.restore_state();
                    }
                    DrawOp::Line { x0, y0, x1, y1, width, color } => {
                        content.save_state();
                        content.set_stroke_rgb(color.r, color.g, color.b);
                        content.set_line_width(*width);
                        content.move_to(*x0, *y0);
                        content.line_to(*x1, *y1);
                        content.stroke();
                        content.restore_state();
                    }
                    DrawOp::Rect { x, y, w, h, fill, stroke } => {
                        content.save_state();
                        content.rect(*x, *y, *w, *h);
                        match (fill, stroke) {
                            (Some(f), Some((sw, sc))) => {
                                content.set_fill_rgb(f.r, f.g, f.b);
                                content.set_stroke_rgb(sc.r, sc.g, sc.b);
                                content.set_line_width(*sw);
                                content.fill_nonzero_and_stroke();
                            }
                            (Some(f), None) => {
                                content.set_fill_rgb(f.r, f.g, f.b);
                                content.fill_nonzero();
                            }
                            (None, Some((sw, sc))) => {
                                content.set_stroke_rgb(sc.r, sc.g, sc.b);
                                content.set_line_width(*sw);
                                content.stroke();
                            }
                            (None, None) => {}
                        }
                        content.restore_state();
                    }
                    DrawOp::Image { image_idx, x, y, w, h } => {
                        let img_name = format!("Im{}", image_idx);
                        content.save_state();
                        content.transform([*w, 0.0, 0.0, *h, *x, *y]);
                        content.x_object(Name(img_name.as_bytes()));
                        content.restore_state();
                    }
                    DrawOp::StationeryPage { stationery_idx } => {
                        let stat_name = format!("Stat{}", stationery_idx);
                        content.save_state();
                        content.transform([page.width_pt, 0.0, 0.0, page.height_pt, 0.0, 0.0]);
                        content.x_object(Name(stat_name.as_bytes()));
                        content.restore_state();
                    }
                }
            }

            let content_data = content.finish();
            pdf.stream(content_refs[pi], &content_data);

            // Page object with inline resources
            let mut page_obj = pdf.page(page_refs[pi]);
            page_obj.parent(page_tree_ref);
            page_obj.media_box(Rect::new(0.0, 0.0, page.width_pt, page.height_pt));
            page_obj.contents(content_refs[pi]);

            // Resources (inline on page)
            let mut resources = page_obj.resources();

            // Font resources
            let mut font_dict = resources.fonts();
            for (fi, _) in self.fonts.iter().enumerate() {
                font_dict.pair(Name(font_names[fi].as_bytes()), font_refs[fi]);
            }
            font_dict.finish();

            // XObject resources (images + stationery)
            let has_xobjects = page.ops.iter().any(|op|
                matches!(op, DrawOp::Image { .. } | DrawOp::StationeryPage { .. })
            );
            if has_xobjects {
                let mut xobjects = resources.x_objects();
                for op in &page.ops {
                    match op {
                        DrawOp::Image { image_idx, .. } => {
                            let name = format!("Im{}", image_idx);
                            xobjects.pair(Name(name.as_bytes()), image_refs[*image_idx]);
                        }
                        DrawOp::StationeryPage { stationery_idx } => {
                            let name = format!("Stat{}", stationery_idx);
                            xobjects.pair(Name(name.as_bytes()), stationery_refs[*stationery_idx]);
                        }
                        _ => {}
                    }
                }
                xobjects.finish();
            }

            resources.finish();
            page_obj.finish();
        }

        // ── Page tree ──
        let mut pages = pdf.pages(page_tree_ref);
        for &pr in &page_refs {
            pages.kids([pr]);
        }
        pages.count(self.pages.len() as i32);
        pages.finish();

        // ── Catalog ──
        pdf.catalog(catalog_ref).pages(page_tree_ref);

        Ok(pdf.finish())
    }
}

/// Encode text as big-endian glyph IDs using the font's cmap.
fn encode_text_to_gids(text: &str, font: &RegisteredFont) -> Vec<u8> {
    let face = ttf_parser::Face::parse(&font.data, 0).ok();
    let mut buf = Vec::new();
    for ch in text.chars() {
        let gid = face
            .as_ref()
            .and_then(|f| f.glyph_index(ch))
            .map(|g| g.0)
            .unwrap_or(0);
        buf.push((gid >> 8) as u8);
        buf.push((gid & 0xFF) as u8);
    }
    buf
}

/// Build a simple identity ToUnicode CMap for text extraction.
fn build_identity_cmap(num_glyphs: u16) -> String {
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\nbegincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");

    // Map in chunks of 100 (PDF spec limit)
    let max = (num_glyphs as u32).min(0xFFFF);
    let mut start = 0u32;
    while start < max {
        let end = (start + 99).min(max - 1);
        let count = end - start + 1;
        cmap.push_str(&format!("{} beginbfrange\n", count));
        for gid in start..=end {
            cmap.push_str(&format!("<{:04X}> <{:04X}> <{:04X}>\n", gid, gid, gid));
        }
        cmap.push_str("endbfrange\n");
        start = end + 1;
    }

    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let c = Color::from_hex("#FF8000");
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
        assert!((c.b - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_color_black() {
        let c = Color::black();
        assert_eq!(c.r, 0.0);
    }

    #[test]
    fn test_add_page() {
        let mut backend = PdfBackend::new();
        let p = backend.add_page(210.0, 297.0);
        assert_eq!(p.0, 0);
        assert_eq!(backend.page_count(), 1);
    }

    #[test]
    fn test_empty_pdf_finish() {
        let mut backend = PdfBackend::new();
        let _p = backend.add_page(210.0, 297.0);
        let bytes = backend.finish().unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 100);
    }
}
