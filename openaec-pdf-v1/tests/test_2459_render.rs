use openaec_pdf_v1::PdfRenderer;
use std::fs;

#[test]
fn test_2459_bitmap_render() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();
    let page = doc.render_page(0, 1.0).unwrap();
    println!("2459 bitmap: {}x{}, {} non-white",
        page.width, page.height,
        page.rgba.chunks(4).filter(|p| p[0]!=255||p[1]!=255||p[2]!=255).count());
    let img = image::RgbaImage::from_raw(page.width, page.height, page.rgba).unwrap();
    img.save("tests/output_2459_bitmap.png").unwrap();
    println!("Saved tests/output_2459_bitmap.png");
}
