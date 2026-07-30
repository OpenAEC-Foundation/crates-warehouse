#[test]
fn test_3037_char_mapping_trace() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) { Ok(b) => b, Err(_) => return };

    let renderer = openaec_pdf_v1::PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();

    // Use the actual font registry to test char_to_glyph_id
    let pdf_doc = lopdf::Document::load_mem(&bytes).unwrap();
    let pages = pdf_doc.get_pages();
    let (_, &page_id) = pages.iter().next().unwrap();
    let page = pdf_doc.get_object(page_id).unwrap().as_dict().unwrap();
    let res = match page.get(b"Resources").unwrap() {
        lopdf::Object::Dictionary(d) => d.clone(),
        lopdf::Object::Reference(id) => pdf_doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };

    let mut font_registry = openaec_pdf_v1::fonts::FontRegistry::new();
    let font_entry = font_registry.get_font("R11", &pdf_doc, &res).unwrap();

    println!("Font R11: base='{}', is_cid={}, encoding={:?}",
        font_entry.base_font, font_entry.is_cid, font_entry.encoding_name);
    println!("ToUnicode entries: {}", font_entry.to_unicode.len());
    println!("Has parsed font: {}", font_entry.parsed.is_some());

    if let Some(ref parsed) = font_entry.parsed {
        println!("Font glyphs: {}, cmap entries: {}, upm: {}",
            parsed.glyphs.len(), parsed.cmap.len(), parsed.units_per_em);
    }

    // Print all ToUnicode mappings
    println!("\nToUnicode mappings:");
    let mut entries: Vec<_> = font_entry.to_unicode.iter().collect();
    entries.sort_by_key(|(k, _)| **k);
    for (&code, &ch) in &entries {
        let gid = openaec_pdf_v1::fonts::FontRegistry::char_to_glyph_id(font_entry, code);
        let has_outline = if let (Some(gid_val), Some(ref parsed)) = (gid, &font_entry.parsed) {
            parsed.glyphs.get(&gid_val).map(|g| !g.commands.is_empty()).unwrap_or(false)
        } else { false };
        println!("  code 0x{:02X} -> U+{:04X} '{}' -> glyph {:?} (has_outline: {})",
            code, ch as u32, ch, gid, has_outline);
    }

    // Test: what happens for "Opdrachtgever"?
    println!("\n=== 'Opdrachtgever' character-by-character ===");
    for ch in "Opdrachtgever".chars() {
        // Find code in ToUnicode that maps to this char
        let code = font_entry.to_unicode.iter().find(|(_, &v)| v == ch).map(|(&k, _)| k);
        if let Some(c) = code {
            let gid = openaec_pdf_v1::fonts::FontRegistry::char_to_glyph_id(font_entry, c);
            println!("  '{}' -> code 0x{:02X} -> glyph {:?}", ch, c, gid);
        } else {
            println!("  '{}' -> NOT IN ToUnicode!", ch);
        }
    }
}
