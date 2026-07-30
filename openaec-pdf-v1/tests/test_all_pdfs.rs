use openaec_pdf_v1::PdfRenderer;
use std::fs;

fn test_pdf(path: &str, name: &str) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("Cannot read {}: {}", name, e));
    let renderer = PdfRenderer::new();
    let doc = match renderer.load_document(&bytes) {
        Ok(d) => d,
        Err(e) => { println!("  âŒ {} â€” load failed: {}", name, e); return; }
    };

    let (w, h) = doc.page_dimensions(0).unwrap_or((0.0, 0.0));
    let start = std::time::Instant::now();
    match doc.render_page(0, 1.0) {
        Ok(page) => {
            let elapsed = start.elapsed();
            let non_white = page.rgba.chunks(4)
                .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
                .count();
            let pct = if page.width * page.height > 0 {
                non_white * 100 / (page.width * page.height) as usize
            } else { 0 };
            let status = if pct > 0 { "âœ…" } else { "âš ï¸ WHITE" };
            println!("  {} {} â€” {}x{} pt, {}x{} px, {:?}, {}% content",
                status, name, w as i32, h as i32, page.width, page.height, elapsed, pct);

            // Save PNG
            let safe_name = name.replace(" ", "_").replace("/", "_");
            let img = image::RgbaImage::from_raw(page.width, page.height, page.rgba).unwrap();
            let _ = img.save(format!("tests/output_{}.png", safe_name));
        }
        Err(e) => {
            println!("  âŒ {} â€” render failed: {}", name, e);
        }
    }
}

#[test]
fn test_all_bouwtekeningen() {
    let dir = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3059 Woonhuis Benedenkerkseweg 87 Stolwijk\20_post_IN\01 27-03-2026 beginstukken";

    println!("\n=== Testing all PDFs in {} ===\n", dir);

    for entry in fs::read_dir(dir).expect("Cannot read directory") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            test_pdf(path.to_str().unwrap(), &name);
        }
    }
}
