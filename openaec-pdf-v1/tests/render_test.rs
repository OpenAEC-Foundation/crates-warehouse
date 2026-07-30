use openaec_pdf_v1::PdfRenderer;

fn minimal_pdf_bytes() -> Vec<u8> {
    use lopdf::{dictionary, Document, Object};

    let mut doc = Document::with_version("1.4");

    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ],
    };
    doc.objects.insert(page_id, Object::Dictionary(page));

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("Failed to save minimal PDF");
    buf
}

#[test]
fn test_load_document() {
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&minimal_pdf_bytes()).unwrap();
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn test_page_dimensions() {
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&minimal_pdf_bytes()).unwrap();
    let (w, h) = doc.page_dimensions(0).unwrap();
    assert!((w - 612.0).abs() < 1.0);
    assert!((h - 792.0).abs() < 1.0);
}

#[test]
fn test_render_returns_rgba() {
    let renderer = PdfRenderer::new();
    let doc = renderer.load_document(&minimal_pdf_bytes()).unwrap();
    let page = doc.render_page(0, 1.0).unwrap();
    assert!(page.width > 0);
    assert!(page.height > 0);
    assert_eq!(page.rgba.len(), (page.width * page.height * 4) as usize);
}
