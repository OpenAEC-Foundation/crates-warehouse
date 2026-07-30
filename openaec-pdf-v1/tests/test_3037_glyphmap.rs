#[test]
fn test_3037_glyph_outline_map() {
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

    // Get embedded font
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

    // Get ToUnicode
    let tu_ref = fdict.get(b"ToUnicode").unwrap();
    let tu_id = match tu_ref { lopdf::Object::Reference(id) => *id, _ => return };
    let tu_stream = match doc.get_object(tu_id).unwrap() {
        lopdf::Object::Stream(ref s) => s.clone(),
        _ => return,
    };
    let cmap_bytes = tu_stream.decompressed_content().unwrap();
    let cmap_str = String::from_utf8_lossy(&cmap_bytes);

    // Parse ToUnicode
    let mut to_unicode = std::collections::HashMap::new();
    let mut in_range = false;
    for line in cmap_str.lines() {
        let line = line.trim();
        if line.contains("beginbfrange") { in_range = true; continue; }
        if line.contains("endbfrange") { in_range = false; continue; }
        if in_range && line.starts_with('<') {
            let hex_values: Vec<u32> = line.split('>').filter_map(|p| {
                let h = p.trim().trim_start_matches('<');
                if h.is_empty() { None } else { u32::from_str_radix(h, 16).ok() }
            }).collect();
            if hex_values.len() >= 3 {
                let lo = hex_values[0] as u8;
                let hi = hex_values[1] as u8;
                let dst = hex_values[2];
                for c in lo..=hi {
                    to_unicode.insert(c, char::from_u32(dst + (c - lo) as u32).unwrap_or('?'));
                }
            }
        }
    }

    // List ALL glyphs with outlines
    struct Counter { count: usize }
    impl ttf_parser::OutlineBuilder for Counter {
        fn move_to(&mut self, _: f32, _: f32) { self.count += 1; }
        fn line_to(&mut self, _: f32, _: f32) { self.count += 1; }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.count += 1; }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.count += 1; }
        fn close(&mut self) { self.count += 1; }
    }

    let mut outline_gids: Vec<u16> = Vec::new();
    for gid in 0..face.number_of_glyphs() {
        let mut c = Counter { count: 0 };
        if face.outline_glyph(ttf_parser::GlyphId(gid), &mut c).is_some() {
            outline_gids.push(gid);
        }
    }

    println!("Glyphs with outlines ({}):", outline_gids.len());
    for &gid in &outline_gids {
        let name = face.glyph_name(ttf_parser::GlyphId(gid));
        println!("  glyph {} = {:?}", gid, name);
    }

    // Now: for each char code, find the CORRECT glyph
    println!("\n=== Required mapping: char_code -> unicode -> glyph ===");
    for code in 1u8..=51 {
        let unicode = to_unicode.get(&code).copied().unwrap_or('?');
        // The correct glyph is the one that ttf-parser would use for this Unicode char
        let cmap_gid = face.glyph_index(unicode);
        println!("  code 0x{:02X} -> '{}' (U+{:04X}) -> cmap: {:?}, direct gid {} has outline: {}",
            code, unicode, unicode as u32,
            cmap_gid.map(|g| g.0),
            code,
            outline_gids.contains(&(code as u16)));
    }
}
