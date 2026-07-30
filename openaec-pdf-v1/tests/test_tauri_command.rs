use openaec_pdf_v1::PdfRenderer;
use std::fs;

/// Simulates exactly what the Tauri command does:
/// read file from disk, load document, render page, return rgba bytes
fn simulate_tauri_render(path: &str, page_index: u32, scale: f32) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read: {}", e))?;
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&bytes).map_err(|e| format!("{}", e))?;
    let page = doc.render_page(page_index as usize, scale).map_err(|e| format!("{}", e))?;
    Ok(page.rgba)
}

fn simulate_tauri_dimensions(path: &str) -> Result<Vec<(f32, f32)>, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read: {}", e))?;
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&bytes).map_err(|e| format!("{}", e))?;
    (0..doc.page_count())
        .map(|i| doc.page_dimensions(i).map_err(|e| format!("{}", e)))
        .collect()
}

#[test]
fn test_tauri_command_simulation() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3059 Woonhuis Benedenkerkseweg 87 Stolwijk\20_post_IN\01 27-03-2026 beginstukken\begane grond do 3 constructie verwerkt_50.pdf";

    // Test get_page_dimensions
    println!("\n--- get_page_dimensions ---");
    let dims = simulate_tauri_dimensions(path).unwrap();
    println!("Pages: {}", dims.len());
    for (i, (w, h)) in dims.iter().enumerate() {
        println!("  Page {}: {}x{} pt", i, w, h);
    }
    assert!(!dims.is_empty());

    // Test render_pdf_page at different scales (simulating DPR=1 and DPR=2)
    for scale in [1.0_f32, 1.5, 2.0, 3.0] {
        println!("\n--- render_pdf_page(scale={}) ---", scale);
        let t0 = std::time::Instant::now();
        let rgba = simulate_tauri_render(path, 0, scale).unwrap();
        let elapsed = t0.elapsed();

        let (w_pt, h_pt) = dims[0];
        let expected_w = (w_pt * scale).ceil() as u32;
        let expected_h = (h_pt * scale).ceil() as u32;
        let actual_pixels = rgba.len() / 4;

        println!("  RGBA bytes: {}", rgba.len());
        println!("  Expected: {}x{} = {} pixels", expected_w, expected_h, expected_w * expected_h);
        println!("  Actual pixels: {}", actual_pixels);
        println!("  Time: {:?}", elapsed);

        // Verify dimensions match
        assert_eq!(rgba.len(), (expected_w * expected_h * 4) as usize,
            "RGBA size mismatch at scale {}! Got {} bytes, expected {}",
            scale, rgba.len(), expected_w * expected_h * 4);

        // Verify content exists
        let non_white = rgba.chunks(4)
            .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
            .count();
        println!("  Non-white pixels: {} ({}%)", non_white, non_white * 100 / actual_pixels);
        assert!(non_white > 0, "Page is white at scale {}", scale);
    }

    // Test that the frontend can construct ImageData from these bytes
    // ImageData requires: new ImageData(Uint8ClampedArray, width, height)
    // width must divide evenly into the array
    let scale = 1.5_f32;
    let rgba = simulate_tauri_render(path, 0, scale).unwrap();
    let (w_pt, _) = dims[0];
    let expected_w = (w_pt * scale).ceil() as u32;
    let expected_h = rgba.len() as u32 / (expected_w * 4);
    
    println!("\n--- Frontend ImageData compatibility ---");
    println!("  Width for ImageData: {}", expected_w);
    println!("  Height from bytes: {}", expected_h);
    println!("  Bytes: {} == {}x{}x4 = {}", rgba.len(), expected_w, expected_h, expected_w * expected_h * 4);
    assert_eq!(rgba.len() as u32, expected_w * expected_h * 4, "ImageData size mismatch!");
    println!("  âœ… Compatible with new ImageData(rgba, {}, {})", expected_w, expected_h);
}
