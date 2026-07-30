#[test]
fn test_3037_font_type() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) { Ok(b) => b, Err(_) => return };
    let doc = lopdf::Document::load_mem(&bytes).unwrap();

    let pages = doc.get_pages();
    let (_, &page_id) = pages.iter().next().unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let res = match page.get(b"Resources").unwrap() {
        lopdf::Object::Dictionary(d) => d.clone(),
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };
    let fonts = match res.get(b"Font").unwrap() {
        lopdf::Object::Dictionary(d) => d.clone(),
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };

    let fref = fonts.get(b"R11").unwrap();
    let fid = match fref { lopdf::Object::Reference(id) => *id, _ => return };
    let fdict = doc.get_object(fid).unwrap().as_dict().unwrap();

    let desc_ref = fdict.get(b"FontDescriptor").unwrap();
    let did = match desc_ref { lopdf::Object::Reference(id) => *id, _ => return };
    let desc = doc.get_object(did).unwrap().as_dict().unwrap();
    let ff2_ref = desc.get(b"FontFile2").unwrap();
    let ff2_id = match ff2_ref { lopdf::Object::Reference(id) => *id, _ => return };
    let ff2_stream = match doc.get_object(ff2_id).unwrap() {
        lopdf::Object::Stream(ref s) => s.clone(),
        _ => return,
    };
    let font_data = ff2_stream.decompressed_content().unwrap();

    // Check magic bytes
    println!("Font data size: {} bytes", font_data.len());
    println!("First 4 bytes: {:02X} {:02X} {:02X} {:02X}", font_data[0], font_data[1], font_data[2], font_data[3]);
    // TrueType: 00 01 00 00 or 'true'
    // OpenType/CFF: 'OTTO'
    // WOFF: 'wOFF'
    let magic = &font_data[0..4];
    if magic == b"OTTO" {
        println!("Type: OpenType with CFF outlines");
    } else if magic == &[0, 1, 0, 0] || magic == b"true" {
        println!("Type: TrueType outlines");
    } else if magic == b"wOFF" {
        println!("Type: WOFF");
    } else {
        println!("Type: Unknown ({:?})", magic);
    }

    let face = ttf_parser::Face::parse(&font_data, 0).unwrap();
    println!("Number of glyphs: {}", face.number_of_glyphs());
    println!("Has glyf table: {}", face.raw_face().table(ttf_parser::Tag::from_bytes(b"glyf")).is_some());
    println!("Has CFF table: {}", face.raw_face().table(ttf_parser::Tag::from_bytes(b"CFF ")).is_some());
    println!("Has CFF2 table: {}", face.raw_face().table(ttf_parser::Tag::from_bytes(b"CFF2")).is_some());
    println!("Has cmap table: {}", face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap")).is_some());

    // Try glyph outline for glyph 1
    let gid = ttf_parser::GlyphId(1);
    let mut builder = DummyBuilder { count: 0 };
    let result = face.outline_glyph(gid, &mut builder);
    println!("\nGlyph 1 outline: {:?}, commands: {}", result.is_some(), builder.count);

    // Try glyph 48 ('O' from Opdrachtgever)
    let gid48 = ttf_parser::GlyphId(48);
    let mut builder48 = DummyBuilder { count: 0 };
    let result48 = face.outline_glyph(gid48, &mut builder48);
    println!("Glyph 48 outline: {:?}, commands: {}", result48.is_some(), builder48.count);

    // Try the glyph for 'F' via cmap
    if let Some(gid_f) = face.glyph_index('F') {
        println!("'F' via cmap: glyph {}", gid_f.0);
        let mut builder_f = DummyBuilder { count: 0 };
        let result_f = face.outline_glyph(gid_f, &mut builder_f);
        println!("'F' outline: {:?}, commands: {}", result_f.is_some(), builder_f.count);
    } else {
        println!("'F' via cmap: NOT FOUND");
    }
}

struct DummyBuilder { count: usize }
impl ttf_parser::OutlineBuilder for DummyBuilder {
    fn move_to(&mut self, _x: f32, _y: f32) { self.count += 1; }
    fn line_to(&mut self, _x: f32, _y: f32) { self.count += 1; }
    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) { self.count += 1; }
    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) { self.count += 1; }
    fn close(&mut self) { self.count += 1; }
}
