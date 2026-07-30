//! Kleur-diagnose: render MV-03 met PDFium Ã©n de tegel-engine op dezelfde
//! schaal, dump beide als PNG en druk de dominante kleuren naast elkaar af
//! (gekwantiseerd histogram, wit/grijs weggefilterd). Verraadt systematische
//! kleurfouten (kanaal-swap, CMYK-conversie, kleurruimte) in Ã©Ã©n oogopslag.

use libloading::{Library, Symbol};
use openaec_pdf_v1::tile_render::TileScene;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int, c_void};

const PDF: &str = "C:/Users/rickd/Documents/GitHub/verification-files/PDF-bestanden/MV-03_Mechanische ventilatie, 3e verdieping ontwerp ACH van 1,5 naar 2,0.pdf";
const DLL: &str = "C:/Users/rickd/Documents/GitHub/open-pdf-studio/open-pdf-studio/src-tauri/binaries/win-x64/pdfium.dll";
const SCALE: f32 = 0.42;

fn pdfium_rgb() -> (u32, u32, Vec<u8>) {
    unsafe {
        let lib = Box::leak(Box::new(Library::new(DLL).expect("dll")));
        let init: Symbol<unsafe extern "C" fn()> = lib.get(b"FPDF_InitLibrary\0").unwrap();
        init();
        let load_doc: Symbol<unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void> =
            lib.get(b"FPDF_LoadDocument\0").unwrap();
        let load_page: Symbol<unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void> =
            lib.get(b"FPDF_LoadPage\0").unwrap();
        let page_w: Symbol<unsafe extern "C" fn(*mut c_void) -> c_double> =
            lib.get(b"FPDF_GetPageWidth\0").unwrap();
        let page_h: Symbol<unsafe extern "C" fn(*mut c_void) -> c_double> =
            lib.get(b"FPDF_GetPageHeight\0").unwrap();
        let bmp_create: Symbol<unsafe extern "C" fn(c_int, c_int, c_int) -> *mut c_void> =
            lib.get(b"FPDFBitmap_Create\0").unwrap();
        let bmp_fill: Symbol<unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_int, u32)> =
            lib.get(b"FPDFBitmap_FillRect\0").unwrap();
        let render: Symbol<unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int, c_int, c_int, c_int, c_int)> =
            lib.get(b"FPDF_RenderPageBitmap\0").unwrap();
        let bmp_buffer: Symbol<unsafe extern "C" fn(*mut c_void) -> *mut c_void> =
            lib.get(b"FPDFBitmap_GetBuffer\0").unwrap();
        let bmp_stride: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            lib.get(b"FPDFBitmap_GetStride\0").unwrap();

        let cpath = CString::new(PDF).unwrap();
        let doc = load_doc(cpath.as_ptr(), std::ptr::null());
        assert!(!doc.is_null());
        let pg = load_page(doc, 0);
        assert!(!pg.is_null());
        let w = (page_w(pg) as f32 * SCALE).ceil() as c_int;
        let h = (page_h(pg) as f32 * SCALE).ceil() as c_int;
        let bmp = bmp_create(w, h, 0);
        bmp_fill(bmp, 0, 0, w, h, 0xFFFF_FFFF);
        render(bmp, pg, 0, 0, w, h, 0, 0);
        let stride = bmp_stride(bmp) as usize;
        let buf = bmp_buffer(bmp) as *const u8;
        let mut rgb = vec![0u8; w as usize * h as usize * 3];
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts(buf.add(y * stride), w as usize * 4);
            for x in 0..w as usize {
                let d = &mut rgb[(y * w as usize + x) * 3..(y * w as usize + x) * 3 + 3];
                d[0] = row[x * 4 + 2];
                d[1] = row[x * 4 + 1];
                d[2] = row[x * 4];
            }
        }
        (w as u32, h as u32, rgb)
    }
}

fn ours_rgb() -> (u32, u32, Vec<u8>) {
    let bytes = std::fs::read(PDF).expect("read");
    let doc = openaec_pdf_v1::DocumentHandle::load(&bytes).expect("load");
    let buf = doc.extract_draw_commands(0, 0).expect("extract");
    let scene = TileScene::build(buf.into_bytes()).expect("scene");
    let page = scene.render_full_parallel(SCALE, 1024);
    let mut rgb = vec![0u8; page.width as usize * page.height as usize * 3];
    for (i, px) in page.rgba.chunks_exact(4).enumerate() {
        let a = px[3] as u32;
        rgb[i * 3] = (px[0] as u32 + (255 - a)).min(255) as u8;
        rgb[i * 3 + 1] = (px[1] as u32 + (255 - a)).min(255) as u8;
        rgb[i * 3 + 2] = (px[2] as u32 + (255 - a)).min(255) as u8;
    }
    (page.width, page.height, rgb)
}

fn top_colors(rgb: &[u8], label: &str) {
    // Kwantiseer naar 32-stappen en filter grijzen (|r-g|,|g-b| klein) + wit.
    let mut counts: HashMap<(u8, u8, u8), u64> = HashMap::new();
    for px in rgb.chunks_exact(3) {
        let (r, g, b) = (px[0], px[1], px[2]);
        let maxc = r.max(g).max(b) as i32;
        let minc = r.min(g).min(b) as i32;
        if maxc - minc < 40 {
            continue; // grijs/zwart/wit â€” niet interessant voor kleurdiagnose
        }
        let q = |v: u8| (v / 32) * 32 + 16;
        *counts.entry((q(r), q(g), q(b))).or_insert(0) += 1;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("{label}: top gekleurde pixels (gekwantiseerd):");
    for ((r, g, b), n) in v.into_iter().take(10) {
        println!("   #{:02X}{:02X}{:02X}  {:>8} px", r, g, b, n);
    }
}

fn save_png(path: &str, w: u32, h: u32, rgb: &[u8]) {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.chunks_exact(3) {
        rgba.extend_from_slice(px);
        rgba.push(255);
    }
    if let Some(pm) = tiny_skia::Pixmap::from_vec(
        premultiply(rgba),
        tiny_skia::IntSize::from_wh(w, h).unwrap(),
    ) {
        let _ = pm.save_png(path);
        println!("png: {}", path);
    }
}

fn premultiply(rgba: Vec<u8>) -> Vec<u8> {
    rgba // alpha=255 overal: al premultiplied
}

fn main() {
    let (rw, rh, r) = pdfium_rgb();
    let (ow, oh, o) = ours_rgb();
    println!("PDFium {}x{}   engine {}x{}\n", rw, rh, ow, oh);
    top_colors(&r, "PDFium ");
    println!();
    top_colors(&o, "engine ");
    save_png("C:/Users/rickd/AppData/Local/Temp/colorprobe_pdfium.png", rw, rh, &r);
    save_png("C:/Users/rickd/AppData/Local/Temp/colorprobe_engine.png", ow, oh, &o);
}
