#[test]
fn test_3037_tounicode_cmap() {
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

    // Get ToUnicode stream
    let tu_ref = fdict.get(b"ToUnicode").unwrap();
    let tu_id = match tu_ref { lopdf::Object::Reference(id) => *id, _ => return };
    let tu_stream = match doc.get_object(tu_id).unwrap() {
        lopdf::Object::Stream(ref s) => s.clone(),
        _ => return,
    };
    let cmap_bytes = tu_stream.decompressed_content().unwrap();
    let cmap_str = String::from_utf8_lossy(&cmap_bytes);

    println!("ToUnicode CMap ({} bytes):", cmap_bytes.len());
    println!("{}", cmap_str);

    // Parse beginbfchar entries: <src> <dst>
    // Also try to look up the Unicode chars in the font's cmap
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

    println!("\n=== Mapping: char_code -> ToUnicode -> cmap glyph_id ===");
    // Parse the CMap manually
    for line in cmap_str.lines() {
        let line = line.trim();
        if line.starts_with('<') && line.contains("> <") {
            let parts: Vec<&str> = line.split("> <").collect();
            if parts.len() == 2 {
                let src = parts[0].trim_start_matches('<');
                let dst = parts[1].trim_end_matches('>');
                if let (Ok(code), Ok(unicode)) = (u32::from_str_radix(src, 16), u32::from_str_radix(dst, 16)) {
                    let ch = char::from_u32(unicode).unwrap_or('?');
                    let gid = face.glyph_index(ch);
                    println!("  code 0x{:02X} -> U+{:04X} '{}' -> glyph {:?}", code, unicode, ch, gid.map(|g| g.0));
                }
            }
        }
    }
}
