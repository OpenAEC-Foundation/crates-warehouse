#[test]
fn find_glyph_f() {
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
    let face = ttf_parser::Face::parse(&font_data, 0).unwrap();

    // Scan ALL glyphs for one that has an outline and looks like 'F'
    // Check glyph names
    struct Counter { count: usize }
    impl ttf_parser::OutlineBuilder for Counter {
        fn move_to(&mut self, _: f32, _: f32) { self.count += 1; }
        fn line_to(&mut self, _: f32, _: f32) { self.count += 1; }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.count += 1; }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.count += 1; }
        fn close(&mut self) { self.count += 1; }
    }

    // Try Windows Symbol encoding: 0xF000 + code
    for code in 0x20u32..=0x7E {
        let ch = char::from_u32(0xF000 + code);
        if let Some(c) = ch {
            if let Some(gid) = face.glyph_index(c) {
                if gid.0 != 0 {
                    let ascii_ch = char::from_u32(code).unwrap_or('?');
                    println!("Symbol 0xF0{:02X} '{}' -> glyph {}", code, ascii_ch, gid.0);
                }
            }
        }
    }

    // Try platform 1 (Mac) encoding
    println!("\nLooking for 'F' (U+0046) in all subtables...");
    // Try different codepoints
    for cp in [0x0046u32, 0xF046, 0x46] {
        if let Some(ch) = char::from_u32(cp) {
            if let Some(gid) = face.glyph_index(ch) {
                println!("  U+{:04X} -> glyph {}", cp, gid.0);
            }
        }
    }

    // Brute force: find ALL glyphs with outlines
    let mut with_outline = 0;
    let mut without_outline = 0;
    for gid in 0..face.number_of_glyphs() {
        let mut c = Counter { count: 0 };
        if face.outline_glyph(ttf_parser::GlyphId(gid), &mut c).is_some() {
            with_outline += 1;
        } else {
            without_outline += 1;
        }
    }
    println!("\nGlyphs with outline: {}, without: {}", with_outline, without_outline);
}
