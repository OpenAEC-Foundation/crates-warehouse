#[test]
fn test_3037_font_cmap_mapping() {
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

    // Get font R11
    let fref = fonts.get(b"R11").unwrap();
    let fid = match fref { lopdf::Object::Reference(id) => *id, _ => return };
    let fdict = doc.get_object(fid).unwrap().as_dict().unwrap();

    // Get embedded font data
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

    // Parse with ttf-parser
    let face = ttf_parser::Face::parse(&font_data, 0).unwrap();
    println!("Number of glyphs: {}", face.number_of_glyphs());

    // Check: what does glyph_index return for various char codes?
    println!("\nDirect char lookup (char code as Unicode):");
    for code in 0x01u32..=0x30 {
        let ch = char::from_u32(code).unwrap_or('\0');
        let gid = face.glyph_index(ch);
        if let Some(g) = gid {
            println!("  char 0x{:02X} ({:?}) -> glyph {}", code, ch, g.0);
        }
    }

    // Check: Windows Symbol encoding (0xF000 + code)
    println!("\nWindows Symbol lookup (0xF000 + code):");
    for code in 0x01u32..=0x30 {
        let ch = char::from_u32(0xF000 + code).unwrap_or('\0');
        let gid = face.glyph_index(ch);
        if let Some(g) = gid {
            println!("  0xF0{:02X} -> glyph {}", code, g.0);
        }
    }

    // Check: what are the glyphs at indices 1-10?
    println!("\nGlyph names at indices 1-10:");
    for gid in 1u16..=10 {
        let id = ttf_parser::GlyphId(gid);
        let name = face.glyph_name(id);
        let has_outline = face.outline_glyph(id, &mut DummyBuilder).is_some();
        println!("  glyph {} = name: {:?}, has_outline: {}", gid, name, has_outline);
    }

    // Check FirstChar/LastChar from PDF
    let first_char = fdict.get(b"FirstChar").and_then(|o| match o {
        lopdf::Object::Integer(i) => Ok(Some(*i)),
        _ => Ok(None),
    }).unwrap();
    let last_char = fdict.get(b"LastChar").and_then(|o| match o {
        lopdf::Object::Integer(i) => Ok(Some(*i)),
        _ => Ok(None),
    }).unwrap();
    println!("\nPDF FirstChar: {:?}, LastChar: {:?}", first_char, last_char);
}

struct DummyBuilder;
impl ttf_parser::OutlineBuilder for DummyBuilder {
    fn move_to(&mut self, _x: f32, _y: f32) {}
    fn line_to(&mut self, _x: f32, _y: f32) {}
    fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {}
    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {}
    fn close(&mut self) {}
}
