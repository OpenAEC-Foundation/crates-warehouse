use openaec_pdf_v1::PdfRenderer;
use std::fs;

#[test]
fn test_2459_bitmap_zoomed() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();
    // Render at 3x scale to see text detail
    let page = doc.render_page(0, 3.0).unwrap();
    println!("2459 3x: {}x{}", page.width, page.height);
    let img = image::RgbaImage::from_raw(page.width, page.height, page.rgba).unwrap();
    // Crop to the text area (bottom-left quadrant)
    let crop = image::imageops::crop_imm(&img, 0, page.height * 2 / 3, page.width / 2, page.height / 3);
    crop.to_image().save("tests/output_2459_text_crop.png").unwrap();
    println!("Saved cropped text area");
}
