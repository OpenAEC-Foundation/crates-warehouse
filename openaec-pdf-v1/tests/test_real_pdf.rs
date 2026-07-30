use openaec_pdf_v1::PdfRenderer;
use std::fs;

#[test]
fn test_real_bouwtekening() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3059 Woonhuis Benedenkerkseweg 87 Stolwijk\20_post_IN\01 27-03-2026 beginstukken\begane grond do 3 constructie verwerkt_50.pdf";
    let bytes = fs::read(path).expect("Could not read test PDF");
    println!("PDF size: {} KB", bytes.len() / 1024);

    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&bytes).expect("Failed to load PDF");

    println!("Pages: {}", doc.page_count());

    let (w, h) = doc.page_dimensions(0).expect("Failed to get dimensions");
    println!("Page 0 dimensions: {}x{} points", w, h);

    let start = std::time::Instant::now();
    let page = doc.render_page(0, 1.0).expect("Failed to render page");
    let elapsed = start.elapsed();

    println!("Rendered: {}x{} pixels in {:?}", page.width, page.height, elapsed);
    println!("RGBA bytes: {}", page.rgba.len());

    // Check it's not all white
    let non_white_pixels = page.rgba.chunks(4)
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count();
    println!("Non-white pixels: {} / {} ({}%)",
        non_white_pixels,
        page.width * page.height,
        non_white_pixels * 100 / (page.width * page.height) as usize
    );

    // Save as PNG for visual inspection
    let img = image::RgbaImage::from_raw(page.width, page.height, page.rgba).unwrap();
    img.save("tests/output_real.png").expect("Failed to save PNG");
    println!("Saved to tests/output_real.png");

    assert!(non_white_pixels > 0, "Page is entirely white - no content rendered!");
}

#[test]
fn test_analyze_page_type() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3059 Woonhuis Benedenkerkseweg 87 Stolwijk\20_post_IN\01 27-03-2026 beginstukken\begane grond do 3 constructie verwerkt_50.pdf";
    let bytes = std::fs::read(path).unwrap();
    let renderer = openaec_pdf_v1::PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();
    let page_type = doc.analyze_page_type(0).unwrap();
    println!("Page type: {:?}", page_type);
}

#[test]
fn test_extract_draw_commands() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3059 Woonhuis Benedenkerkseweg 87 Stolwijk\20_post_IN\01 27-03-2026 beginstukken\begane grond do 3 constructie verwerkt_50.pdf";
    let bytes = std::fs::read(path).unwrap();
    let renderer = openaec_pdf_v1::PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();

    let t0 = std::time::Instant::now();
    let cmds = doc.extract_draw_commands(0).unwrap();
    let elapsed = t0.elapsed();

    let data = cmds.into_bytes();
    println!("Draw commands: {} bytes ({} KB) in {:?}", data.len(), data.len() / 1024, elapsed);
    assert!(data.len() > 8, "Should have commands beyond header");
}
