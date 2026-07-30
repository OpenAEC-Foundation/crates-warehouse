//! Corpus-pixel-diff: onze parallelle tegel-rasterizer vs PDFium-referentie,
//! over alle testbestanden. Gate voor de zwaar-blad-router (fase 2, trap b).
//!
//! Referentie: pdfium.dll via libloading (raw FPDF), BGRA â†’ wit-composiet RGB.
//! Kandidaat: extract_draw_commands â†’ TileScene (512px-tegels) â†’ wit-composiet.
//! Metriek per pagina: % pixels met max-kanaal-delta > 16, gemiddelde delta.
//!
//! Gebruik: corpus_diff <map-of-pdf> [nog een map/pdf ...]
//!
//! Dump-modus (galerij voor het meetrapport): zet CORPUS_DUMP_DIR=<map>.
//! Per gemeten pagina komen daar drie PNG's (max 700 px lange zijde):
//!   <stem>_pN_ref.png   PDFium-referentie ("hoe het hoort")
//!   <stem>_pN_ours.png  eigen engine ("wat hij toont")
//!   <stem>_pN_diff.png  referentie gedimd, verschilpixels felrood
//! plus volledige-resolutie drieluiken (_full) voor de beste en slechtste
//! pagina per bestand, en gallery.json met alle metingen Ã©n de router-keuze
//! (JS-voorfilter content-bytes + Rust-gate images/clips â€” zelfde drempels
//! als de app). Optioneel: CORPUS_PDFIUM_DLL overschrijft het dll-pad.

use libloading::{Library, Symbol};
use openaec_pdf_v1::tile_render::TileScene;
use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::{Path, PathBuf};
use std::time::Instant;

const TARGET_LONG_PX: f32 = 1400.0;
const DELTA_THRESH: i32 = 16;
const THUMB_LONG_PX: u32 = 700;

// Router-drempels â€” spiegel van de app:
//   voorfilter: js/pdf/progressive-render.js (SCENE_CONTENT_BYTES)
//   gate:       src-tauri/src/lib.rs (scene-weigering op buffer-feiten)
const SCENE_CONTENT_BYTES: u64 = 6_000_000;
const GATE_BUFFER_MAX: usize = 400 * 1024 * 1024;
// 1 MB -> 2 MB (2026-07-06): rasterizer tekent images nu echt; de inline-
// strip-emissie tilt MV-03 (1,44 MB payload, correct beeld) over de oude
// drempel. Spiegel van src-tauri/src/lib.rs â€” daar staat de onderbouwing.
const GATE_IMAGE_BYTES: u64 = 2_000_000;
const GATE_CLIPS_PER_MB: u64 = 25;

struct Fpdf {
    _lib: &'static Library,
    load_doc: Symbol<'static, unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void>,
    close_doc: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
    page_count: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> c_int>,
    load_page: Symbol<'static, unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void>,
    close_page: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
    page_w: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> c_double>,
    page_h: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> c_double>,
    bmp_create: Symbol<'static, unsafe extern "C" fn(c_int, c_int, c_int) -> *mut c_void>,
    bmp_destroy: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
    bmp_fill: Symbol<'static, unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_int, u32)>,
    render: Symbol<'static, unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int, c_int, c_int, c_int, c_int)>,
    bmp_buffer: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    bmp_stride: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> c_int>,
}

impl Fpdf {
    fn load(dll: &str) -> Result<Self, String> {
        unsafe {
            let lib = Box::leak(Box::new(Library::new(dll).map_err(|e| e.to_string())?));
            let init: Symbol<unsafe extern "C" fn()> =
                lib.get(b"FPDF_InitLibrary\0").map_err(|e| e.to_string())?;
            init();
            Ok(Fpdf {
                load_doc: lib.get(b"FPDF_LoadDocument\0").map_err(|e| e.to_string())?,
                close_doc: lib.get(b"FPDF_CloseDocument\0").map_err(|e| e.to_string())?,
                page_count: lib.get(b"FPDF_GetPageCount\0").map_err(|e| e.to_string())?,
                load_page: lib.get(b"FPDF_LoadPage\0").map_err(|e| e.to_string())?,
                close_page: lib.get(b"FPDF_ClosePage\0").map_err(|e| e.to_string())?,
                page_w: lib.get(b"FPDF_GetPageWidth\0").map_err(|e| e.to_string())?,
                page_h: lib.get(b"FPDF_GetPageHeight\0").map_err(|e| e.to_string())?,
                bmp_create: lib.get(b"FPDFBitmap_Create\0").map_err(|e| e.to_string())?,
                bmp_destroy: lib.get(b"FPDFBitmap_Destroy\0").map_err(|e| e.to_string())?,
                bmp_fill: lib.get(b"FPDFBitmap_FillRect\0").map_err(|e| e.to_string())?,
                render: lib.get(b"FPDF_RenderPageBitmap\0").map_err(|e| e.to_string())?,
                bmp_buffer: lib.get(b"FPDFBitmap_GetBuffer\0").map_err(|e| e.to_string())?,
                bmp_stride: lib.get(b"FPDFBitmap_GetStride\0").map_err(|e| e.to_string())?,
                _lib: lib,
            })
        }
    }

    /// Render pagina â†’ RGB op witte achtergrond + (w, h, pt-afmetingen).
    fn render_rgb(&self, path: &str, page: usize, scale: f32) -> Result<(u32, u32, Vec<u8>), String> {
        unsafe {
            let cpath = CString::new(path).map_err(|e| e.to_string())?;
            let doc = (self.load_doc)(cpath.as_ptr(), std::ptr::null());
            if doc.is_null() {
                return Err("FPDF_LoadDocument null".into());
            }
            let n = (self.page_count)(doc);
            if page as c_int >= n {
                (self.close_doc)(doc);
                return Err(format!("pagina {} > count {}", page, n));
            }
            let pg = (self.load_page)(doc, page as c_int);
            if pg.is_null() {
                (self.close_doc)(doc);
                return Err("FPDF_LoadPage null".into());
            }
            let w = ((self.page_w)(pg) as f32 * scale).ceil().max(1.0) as c_int;
            let h = ((self.page_h)(pg) as f32 * scale).ceil().max(1.0) as c_int;
            let bmp = (self.bmp_create)(w, h, 0);
            if bmp.is_null() {
                (self.close_page)(pg);
                (self.close_doc)(doc);
                return Err("FPDFBitmap_Create null".into());
            }
            (self.bmp_fill)(bmp, 0, 0, w, h, 0xFFFF_FFFF);
            (self.render)(bmp, pg, 0, 0, w, h, 0, 0);
            let stride = (self.bmp_stride)(bmp) as usize;
            let buf = (self.bmp_buffer)(bmp) as *const u8;
            let mut rgb = vec![0u8; w as usize * h as usize * 3];
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts(buf.add(y * stride), w as usize * 4);
                for x in 0..w as usize {
                    // BGRA (opaque na FillRect) â†’ RGB
                    let d = &mut rgb[(y * w as usize + x) * 3..(y * w as usize + x) * 3 + 3];
                    d[0] = row[x * 4 + 2];
                    d[1] = row[x * 4 + 1];
                    d[2] = row[x * 4];
                }
            }
            (self.bmp_destroy)(bmp);
            (self.close_page)(pg);
            (self.close_doc)(doc);
            Ok((w as u32, h as u32, rgb))
        }
    }
}

/// Onze kant: premultiplied RGBA â†’ wit-composiet RGB.
fn ours_rgb(page: &openaec_pdf_v1::RenderedPage) -> Vec<u8> {
    let mut rgb = vec![0u8; page.width as usize * page.height as usize * 3];
    for (i, px) in page.rgba.chunks_exact(4).enumerate() {
        let a = px[3] as u32;
        // premultiplied over wit: c + (255 - a)
        rgb[i * 3] = (px[0] as u32 + (255 - a)).min(255) as u8;
        rgb[i * 3 + 1] = (px[1] as u32 + (255 - a)).min(255) as u8;
        rgb[i * 3 + 2] = (px[2] as u32 + (255 - a)).min(255) as u8;
    }
    rgb
}

fn collect_pdfs(args: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for a in args {
        let p = Path::new(a);
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    let f = e.path();
                    if f.extension().map_or(false, |x| x.eq_ignore_ascii_case("pdf")) {
                        out.push(f);
                    }
                }
            }
        } else if p.is_file() {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out
}

/// Opcode-telling over de display-list (zelfde byte-walk als examples/op_histo.rs).
/// Voedt de duiding in het rapport: waar komt een afwijking vandaan
/// (lijnwerk, arceringen/clips, tekst of embedded images)?
#[derive(Default, Clone, Copy)]
struct OpCounts {
    segs: u64,    // MoveTo/LineTo/CubicTo/Rect
    strokes: u64, // Stroke
    fills: u64,   // Fill + FillEO
    texts: u64,   // TextAt
    images: u64,  // DrawImage
    clips: u64,   // Clip + ClipEO
}

fn op_counts(d: &[u8]) -> OpCounts {
    let mut c = OpCounts::default();
    let mut pos = 16usize;
    while pos < d.len() {
        let op = d[pos];
        if op > 21 {
            break; // onbekende opcode: telling stoppen, niet gokken
        }
        pos += 1;
        match op {
            0..=3 => c.segs += 1,
            7 => c.strokes += 1,
            8 | 9 => c.fills += 1,
            18 => c.texts += 1,
            19 => c.images += 1,
            20 | 21 => c.clips += 1,
            _ => {}
        }
        let need = match op {
            0 | 1 => 8,
            2 | 12 => 24,
            3 => 16,
            5 => 8,
            6 => 4,
            13 | 14 => 1,
            15 => 4,
            16 => {
                if pos >= d.len() { break; }
                let n = d[pos] as usize;
                1 + n * 4 + 4
            }
            18 => {
                if pos + 17 > d.len() { break; }
                let len = d[pos + 12 + 4] as usize;
                12 + 4 + 1 + len
            }
            19 => {
                if pos + 8 > d.len() { break; }
                let dlen = u32::from_le_bytes(d[pos + 4..pos + 8].try_into().unwrap()) as usize;
                8 + dlen
            }
            _ => 0,
        };
        pos += need;
    }
    c
}

/// Gecomprimeerde content-stream-bytes van een pagina â€” zelfde goedkope
/// zwaarte-probe als het Tauri-command page_content_size (het JS-voorfilter).
fn content_stream_bytes(doc: &lopdf::Document, page_index: usize) -> u64 {
    use lopdf::Object;
    let pages = doc.get_pages();
    let Some(page_id) = pages.values().nth(page_index).copied() else { return 0 };
    let Ok(page) = doc.get_dictionary(page_id) else { return 0 };
    let Ok(contents) = page.get(b"Contents") else { return 0 };
    let mut ids: Vec<lopdf::ObjectId> = Vec::new();
    match contents {
        Object::Reference(id) => ids.push(*id),
        Object::Array(arr) => {
            for o in arr {
                if let Object::Reference(id) = o {
                    ids.push(*id);
                }
            }
        }
        _ => {}
    }
    ids.iter()
        .filter_map(|id| match doc.get_object(*id) {
            Ok(Object::Stream(s)) => Some(s.content.len() as u64),
            _ => None,
        })
        .sum()
}

/// De engine-keuze van de app, nagespeeld op de gemeten feiten:
///   1) JS-voorfilter: gecomprimeerde content-stream >= 6 MB â†’ kandidaat,
///      anders PDFium (de basis-engine).
///   2) Rust-gate: buffer > 400 MB, > 1 MB embedded images of > 25 clips/MB
///      â†’ weigering, pagina valt blijvend terug op PDFium.
fn router_choice(content: u64, buffer_len: usize, image_bytes: u64, clip_ops: u64) -> (&'static str, String) {
    if content < SCENE_CONTENT_BYTES {
        return ("PDFium", format!("voorfilter: content {:.1} MB < 6 MB", content as f64 / 1e6));
    }
    if buffer_len > GATE_BUFFER_MAX {
        return ("PDFium", format!("gate: scene {} MB > 400 MB", buffer_len / 1_048_576));
    }
    if image_bytes > GATE_IMAGE_BYTES {
        return ("PDFium", format!("gate: {:.1} MB embedded images > 1 MB", image_bytes as f64 / 1e6));
    }
    let mb = ((buffer_len / 1_048_576) as u64).max(1);
    if clip_ops / mb > GATE_CLIPS_PER_MB {
        return ("PDFium", format!("gate: {} clips/MB > 25", clip_ops / mb));
    }
    ("AEC-PDF v1", format!("content {:.1} MB â‰¥ 6 MB en gate ok", content as f64 / 1e6))
}

fn save_png_rgb(path: &Path, w: u32, h: u32, rgb: &[u8]) {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.chunks_exact(3) {
        rgba.extend_from_slice(px);
        rgba.push(255); // alpha 255 â†’ premultiplied == straight
    }
    match tiny_skia::IntSize::from_wh(w, h).and_then(|s| tiny_skia::Pixmap::from_vec(rgba, s)) {
        Some(pm) => {
            if let Err(e) = pm.save_png(path) {
                println!("png-fout {}: {}", path.display(), e);
            }
        }
        None => println!("png-fout {}: pixmap {}x{}", path.display(), w, h),
    }
}

/// Box-downsample naar max `long` px lange zijde (gehele factor).
/// keep_red: een blok met minstens Ã©Ã©n felrode diff-pixel blijft felrood,
/// zodat dunne verschillijnen de verkleining overleven.
fn thumb_rgb(w: u32, h: u32, rgb: &[u8], long: u32, keep_red: bool) -> (u32, u32, Vec<u8>) {
    let f = ((w.max(h) + long - 1) / long).max(1);
    if f == 1 {
        return (w, h, rgb.to_vec());
    }
    let tw = (w + f - 1) / f;
    let th = (h + f - 1) / f;
    let mut out = vec![0u8; tw as usize * th as usize * 3];
    for by in 0..th {
        for bx in 0..tw {
            let (mut rs, mut gs, mut bs, mut n) = (0u32, 0u32, 0u32, 0u32);
            let mut red = false;
            for y in by * f..(by * f + f).min(h) {
                for x in bx * f..(bx * f + f).min(w) {
                    let i = (y as usize * w as usize + x as usize) * 3;
                    let (r, g, b) = (rgb[i], rgb[i + 1], rgb[i + 2]);
                    if keep_red && r == 255 && g == 0 && b == 0 {
                        red = true;
                    }
                    rs += r as u32;
                    gs += g as u32;
                    bs += b as u32;
                    n += 1;
                }
            }
            let o = (by as usize * tw as usize + bx as usize) * 3;
            if red {
                out[o] = 255;
            } else if n > 0 {
                out[o] = (rs / n) as u8;
                out[o + 1] = (gs / n) as u8;
                out[o + 2] = (bs / n) as u8;
            }
        }
    }
    (tw, th, out)
}

/// Diff-visualisatie: referentie gedimd, pixels met max-kanaal-delta > 16 felrood.
fn diff_vis_rgb(refr: &[u8], ours: &[u8]) -> Vec<u8> {
    let mut vis = vec![0u8; refr.len()];
    for i in (0..refr.len()).step_by(3) {
        let d = (0..3)
            .map(|c| (refr[i + c] as i32 - ours[i + c] as i32).abs())
            .max()
            .unwrap();
        if d > DELTA_THRESH {
            vis[i] = 255;
        } else {
            vis[i] = refr[i] / 3 + 170;
            vis[i + 1] = refr[i + 1] / 3 + 170;
            vis[i + 2] = refr[i + 2] / 3 + 170;
        }
    }
    vis
}

/// Bestandsnaam-veilige stam met vast volgnummer: "f03_3131-clt-set".
fn safe_stem(name: &str, idx: usize) -> String {
    let base = name.strip_suffix(".pdf").or_else(|| name.strip_suffix(".PDF")).unwrap_or(name);
    let mut s = String::new();
    for c in base.chars() {
        let c = c.to_ascii_lowercase();
        let mapped = if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' };
        if mapped == '_' && s.ends_with('_') {
            continue;
        }
        s.push(mapped);
        if s.len() >= 40 {
            break;
        }
    }
    format!("f{:02}_{}", idx + 1, s.trim_matches('_'))
}

fn jesc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Volledige meting van Ã©Ã©n pagina, voor gallery.json.
struct PageRow {
    page: usize,
    w: u32,
    h: u32,
    scale: f32,
    content: u64,
    buffer: usize,
    image_bytes: u64,
    clip_ops: u64,
    ops: OpCounts,
    ref_ms: u128,
    extract_ms: u128,
    index_ms: u128,
    render_ms: u128,
    pct: f64,
    ds_pct: f64,
    ink_ref_pct: f64,
    ink_ours_pct: f64,
    engine: &'static str,
    reason: String,
}

impl PageRow {
    fn to_json(&self) -> String {
        format!(
            "{{\"pagina\":{},\"w_px\":{},\"h_px\":{},\"schaal\":{:.4},\"content_bytes\":{},\"buffer_bytes\":{},\"image_bytes\":{},\"clip_ops\":{},\"ops\":{{\"segmenten\":{},\"strokes\":{},\"fills\":{},\"teksten\":{},\"images\":{},\"clips\":{}}},\"pdfium_ms\":{},\"extract_ms\":{},\"index_ms\":{},\"render_ms\":{},\"diff_pct\":{:.3},\"ds_diff_pct\":{:.3},\"inkt_ref_pct\":{:.3},\"inkt_ours_pct\":{:.3},\"engine\":\"{}\",\"reden\":\"{}\"}}",
            self.page, self.w, self.h, self.scale, self.content, self.buffer, self.image_bytes,
            self.clip_ops, self.ops.segs, self.ops.strokes, self.ops.fills, self.ops.texts,
            self.ops.images, self.ops.clips, self.ref_ms, self.extract_ms, self.index_ms,
            self.render_ms, self.pct, self.ds_pct, self.ink_ref_pct, self.ink_ours_pct,
            self.engine, jesc(&self.reason)
        )
    }
}

/// Kandidaat voor het volle-resolutie-drieluik (beste/slechtste pagina).
struct FullCand {
    ds: f64,
    page: usize,
    w: u32,
    h: u32,
    refr: Vec<u8>,
    ours: Vec<u8>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dll_env = std::env::var("CORPUS_PDFIUM_DLL").unwrap_or_default();
    let dll_candidates = [
        dll_env.as_str(),
        "C:/Users/rickd/Documents/GitHub/open-pdf-studio/open-pdf-studio/src-tauri/binaries/win-x64/pdfium.dll",
        "C:/Users/rickd/AppData/Local/Temp/opds-tile-wt/open-pdf-studio/src-tauri/binaries/win-x64/pdfium.dll",
    ];
    let dll = dll_candidates
        .iter()
        .copied()
        .find(|p| !p.is_empty() && Path::new(p).is_file())
        .expect("geen pdfium.dll gevonden (zet CORPUS_PDFIUM_DLL)");
    let fpdf = Fpdf::load(dll).expect("pdfium.dll laden");

    let dump_dir: Option<PathBuf> = std::env::var("CORPUS_DUMP_DIR").ok().map(PathBuf::from);
    if let Some(d) = &dump_dir {
        std::fs::create_dir_all(d).expect("dump-map aanmaken");
        println!("dump-modus: {}\n", d.display());
    }

    let pdfs = collect_pdfs(&args);
    println!("corpus: {} bestanden\n", pdfs.len());
    println!("{:<44} {:>5} {:>9} {:>9} {:>8} {:>8} {:>8}", "bestand", "pag", "ref ms", "tile ms", ">16 %", "gem d", "status");

    let mut worst: Vec<(f64, String)> = Vec::new();
    let mut csv = String::from("bestand	pagina	content_bytes	buffer_bytes	ref_ms	extract_ms	index_ms	render_ms	diff_pct	ds_diff_pct	inkt_ref_pct	inkt_ours_pct	engine
");
    let mut files_json: Vec<String> = Vec::new();
    for (fi, pdf) in pdfs.iter().enumerate() {
        let name = pdf.file_name().unwrap().to_string_lossy().to_string();
        let short: String = name.chars().take(42).collect();
        let stem = safe_stem(&name, fi);
        let bytes = match std::fs::read(pdf) {
            Ok(b) => b,
            Err(e) => { println!("{:<44} {:>5} lees-fout: {}", short, "-", e); continue; }
        };
        let file_bytes = bytes.len();

        // Kandidaat-document Ã©Ã©n keer laden
        let doc = match openaec_pdf_v1::DocumentHandle::load(&bytes) {
            Ok(d) => d,
            Err(e) => {
                println!("{:<44} {:>5} onze load-fout: {:?}", short, "-", e);
                if dump_dir.is_some() {
                    files_json.push(format!(
                        "{{\"bestand\":\"{}\",\"stam\":\"{}\",\"bestand_bytes\":{},\"fout\":\"load: {}\"}}",
                        jesc(&name), stem, file_bytes, jesc(&format!("{:?}", e))
                    ));
                }
                continue;
            }
        };
        // lopdf-parse voor de content-stream-probe (het JS-voorfilter van de router)
        let lodoc = lopdf::Document::load_mem(&bytes).ok();
        let pages_total = doc.page_count();
        let test_pages: Vec<usize> = (0..pages_total.min(8)).collect();

        let mut rows: Vec<PageRow> = Vec::new();
        let mut fouten: Vec<String> = Vec::new();
        let mut worst_c: Option<FullCand> = None;
        let mut best_c: Option<FullCand> = None;

        for &pg in &test_pages {
            let (w_pt, h_pt) = match doc.page_dimensions(pg) {
                Ok(d) => d,
                Err(e) => { println!("{:<44} {:>5} dims-fout: {:?}", short, pg + 1, e); fouten.push(format!("p{}: dims {:?}", pg + 1, e)); continue; }
            };
            let scale = (TARGET_LONG_PX / w_pt.max(h_pt)).min(2.0);

            let tr = Instant::now();
            let refr = match fpdf.render_rgb(&pdf.to_string_lossy(), pg, scale) {
                Ok(r) => r,
                Err(e) => { println!("{:<44} {:>5} ref-fout: {}", short, pg + 1, e); fouten.push(format!("p{}: ref {}", pg + 1, e)); continue; }
            };
            let ref_ms = tr.elapsed().as_millis();

            let te = Instant::now();
            let extracted = doc.extract_draw_commands(pg, 0).map(|b| b.into_bytes());
            let extract_ms = te.elapsed().as_millis();
            let (scene, buf_len, index_ms, ops) = match extracted {
                Ok(bytes) => {
                    let n = bytes.len();
                    let ops = op_counts(&bytes);
                    let ti = Instant::now();
                    match TileScene::build(bytes) {
                        Ok(sc) => (Some(sc), n, ti.elapsed().as_millis(), ops),
                        Err(e) => { println!("{:<44} {:>5} scene-fout: {}", short, pg + 1, e); fouten.push(format!("p{}: scene {}", pg + 1, e)); continue; }
                    }
                }
                Err(e) => { println!("{:<44} {:>5} extract-fout: {:?}", short, pg + 1, e); fouten.push(format!("p{}: extract {:?}", pg + 1, e)); continue; }
            };
            let scene = scene.unwrap();
            let tr2 = Instant::now();
            let ours = scene.render_full_parallel(scale, 512);
            let render_ms = tr2.elapsed().as_millis();
            let tile_ms = extract_ms + index_ms + render_ms;

            if ours.width != refr.0 || ours.height != refr.1 {
                println!("{:<44} {:>5} maten verschillen: {}x{} vs {}x{}", short, pg + 1, ours.width, ours.height, refr.0, refr.1);
                fouten.push(format!("p{}: maten {}x{} vs {}x{}", pg + 1, ours.width, ours.height, refr.0, refr.1));
                continue;
            }
            let ours_rgb = ours_rgb(&ours);
            let total = (refr.0 * refr.1) as usize;
            let mut over = 0usize;
            let mut sum: u64 = 0;
            // Inktdekking: pixels die zichtbaar niet-wit zijn (kanaal < 245).
            // ours << ref verraadt een (vrijwel) lege render; ours >> ref
            // verraadt niet-weggeclipte vlakken. Voedt de duiding per blad.
            let (mut ink_ref, mut ink_ours) = (0usize, 0usize);
            for (a, b) in refr.2.chunks_exact(3).zip(ours_rgb.chunks_exact(3)) {
                let d = (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).max().unwrap();
                sum += d as u64;
                if d > DELTA_THRESH { over += 1; }
                if a.iter().any(|&c| c < 245) { ink_ref += 1; }
                if b.iter().any(|&c| c < 245) { ink_ours += 1; }
            }
            let pct = 100.0 * over as f64 / total as f64;
            let mean = sum as f64 / total as f64;
            // Downsampled metriek (4x4-box): meet 'inkt per gebied' en negeert
            // AA-verdelingsverschillen tussen rasterizers.
            let dw = (refr.0 / 4).max(1) as usize;
            let dh = (refr.1 / 4).max(1) as usize;
            let mut ds_over = 0usize;
            for by in 0..dh {
                for bx in 0..dw {
                    let (mut ra, mut ga, mut ba, mut rb, mut gb, mut bb, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
                    for y in by * 4..((by * 4 + 4).min(refr.1 as usize)) {
                        for x in bx * 4..((bx * 4 + 4).min(refr.0 as usize)) {
                            let i = (y * refr.0 as usize + x) * 3;
                            ra += refr.2[i] as u32; ga += refr.2[i + 1] as u32; ba += refr.2[i + 2] as u32;
                            rb += ours_rgb[i] as u32; gb += ours_rgb[i + 1] as u32; bb += ours_rgb[i + 2] as u32;
                            n += 1;
                        }
                    }
                    if n == 0 { continue; }
                    let d = ((ra / n) as i32 - (rb / n) as i32).abs()
                        .max(((ga / n) as i32 - (gb / n) as i32).abs())
                        .max(((ba / n) as i32 - (bb / n) as i32).abs());
                    if d > 12 { ds_over += 1; }
                }
            }
            let ds_pct = 100.0 * ds_over as f64 / (dw * dh) as f64;
            let _ = mean;
            let ink_ref_pct = 100.0 * ink_ref as f64 / total as f64;
            let ink_ours_pct = 100.0 * ink_ours as f64 / total as f64;
            let content = lodoc.as_ref().map(|d| content_stream_bytes(d, pg)).unwrap_or(0);
            let (engine, reason) = router_choice(content, buf_len, scene.image_bytes, scene.clip_ops);
            let status = if ds_pct <= 2.0 { "OK" } else { "AFWIJKEND" };
            println!("{:<44} {:>5} {:>9} {:>9} {:>7.2}% {:>7.2}% {:>8}", short, pg + 1, ref_ms, tile_ms, pct, ds_pct, status);
            csv.push_str(&format!("{}	{}	{}	{}	{}	{}	{}	{}	{:.3}	{:.3}	{:.3}	{:.3}	{}
",
                name, pg + 1, content, buf_len, ref_ms, extract_ms, index_ms, render_ms, pct, ds_pct, ink_ref_pct, ink_ours_pct, engine));
            worst.push((ds_pct, format!("{} p{}", name, pg + 1)));

            if let Some(dir) = &dump_dir {
                // Drie thumbnails per pagina (halve resolutie, max 700 px lange zijde).
                let (tw, th, tref) = thumb_rgb(refr.0, refr.1, &refr.2, THUMB_LONG_PX, false);
                save_png_rgb(&dir.join(format!("{}_p{}_ref.png", stem, pg + 1)), tw, th, &tref);
                let (_, _, tours) = thumb_rgb(refr.0, refr.1, &ours_rgb, THUMB_LONG_PX, false);
                save_png_rgb(&dir.join(format!("{}_p{}_ours.png", stem, pg + 1)), tw, th, &tours);
                let dv = diff_vis_rgb(&refr.2, &ours_rgb);
                let (_, _, tdiff) = thumb_rgb(refr.0, refr.1, &dv, THUMB_LONG_PX, true);
                save_png_rgb(&dir.join(format!("{}_p{}_diff.png", stem, pg + 1)), tw, th, &tdiff);

                // Kandidaten voor het volle-resolutie-drieluik bijhouden.
                let mk = |ds: f64| FullCand { ds, page: pg + 1, w: refr.0, h: refr.1, refr: refr.2.clone(), ours: ours_rgb.clone() };
                if worst_c.as_ref().map_or(true, |c| ds_pct > c.ds) {
                    worst_c = Some(mk(ds_pct));
                }
                if best_c.as_ref().map_or(true, |c| ds_pct < c.ds) {
                    best_c = Some(mk(ds_pct));
                }

                rows.push(PageRow {
                    page: pg + 1,
                    w: refr.0,
                    h: refr.1,
                    scale,
                    content,
                    buffer: buf_len,
                    image_bytes: scene.image_bytes,
                    clip_ops: scene.clip_ops,
                    ops,
                    ref_ms,
                    extract_ms,
                    index_ms,
                    render_ms,
                    pct,
                    ds_pct,
                    ink_ref_pct,
                    ink_ours_pct,
                    engine,
                    reason,
                });
            }
        }

        if let Some(dir) = &dump_dir {
            // Volle resolutie voor de slechtste Ã©n beste pagina (klikbare originelen).
            let write_full = |c: &FullCand| {
                save_png_rgb(&dir.join(format!("{}_p{}_ref_full.png", stem, c.page)), c.w, c.h, &c.refr);
                save_png_rgb(&dir.join(format!("{}_p{}_ours_full.png", stem, c.page)), c.w, c.h, &c.ours);
                let dv = diff_vis_rgb(&c.refr, &c.ours);
                save_png_rgb(&dir.join(format!("{}_p{}_diff_full.png", stem, c.page)), c.w, c.h, &dv);
            };
            let worst_page = worst_c.as_ref().map(|c| c.page);
            let best_page = best_c.as_ref().map(|c| c.page);
            if let Some(c) = &worst_c { write_full(c); }
            if let (Some(c), true) = (&best_c, best_page != worst_page) { write_full(c); }

            let pages_json: Vec<String> = rows.iter().map(|r| r.to_json()).collect();
            let fouten_json: Vec<String> = fouten.iter().map(|f| format!("\"{}\"", jesc(f))).collect();
            files_json.push(format!(
                "{{\"bestand\":\"{}\",\"stam\":\"{}\",\"bestand_bytes\":{},\"paginas_totaal\":{},\"slechtste_pagina\":{},\"beste_pagina\":{},\"fouten\":[{}],\"paginas\":[\n{}\n]}}",
                jesc(&name), stem, file_bytes, pages_total,
                worst_page.map_or("null".into(), |p| p.to_string()),
                best_page.map_or("null".into(), |p| p.to_string()),
                fouten_json.join(","),
                pages_json.join(",\n")
            ));
        }
    }
    let _ = std::fs::write("C:/Users/rickd/AppData/Local/Temp/corpus_bench.tsv", &csv);
    println!("
csv: C:/Users/rickd/AppData/Local/Temp/corpus_bench.tsv");
    if let Some(dir) = &dump_dir {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let json = format!(
            "{{\n\"gegenereerd_unix\":{},\n\"doel_lange_zijde_px\":{},\n\"thumb_lange_zijde_px\":{},\n\"delta_drempel\":{},\n\"drempels\":{{\"voorfilter_content_bytes\":{},\"gate_buffer_max_bytes\":{},\"gate_image_bytes\":{},\"gate_clips_per_mb\":{}}},\n\"bestanden\":[\n{}\n]\n}}\n",
            unix, TARGET_LONG_PX, THUMB_LONG_PX, DELTA_THRESH,
            SCENE_CONTENT_BYTES, GATE_BUFFER_MAX, GATE_IMAGE_BYTES, GATE_CLIPS_PER_MB,
            files_json.join(",\n")
        );
        let jpath = dir.join("gallery.json");
        std::fs::write(&jpath, json).expect("gallery.json schrijven");
        println!("json: {}", jpath.display());
    }
    worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("\nslechtste 5:");
    for (pct, label) in worst.iter().take(5) {
        println!("  {:>6.2}%  {}", pct, label);
    }
}
