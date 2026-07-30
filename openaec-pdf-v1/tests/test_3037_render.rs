#[test]
fn test_3037_content_stream_operators() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => { println!("File not found"); return; }
    };
    let doc = lopdf::Document::load_mem(&bytes).unwrap();
    let pages = doc.get_pages();
    let (_, &page_id) = pages.iter().next().unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();

    // Get content stream
    let contents = page.get(b"Contents").unwrap();
    let mut all_bytes = Vec::new();
    match contents {
        lopdf::Object::Reference(id) => {
            if let Ok(lopdf::Object::Stream(ref s)) = doc.get_object(*id) {
                all_bytes = s.decompressed_content().unwrap();
            }
        }
        lopdf::Object::Array(arr) => {
            for item in arr {
                if let lopdf::Object::Reference(id) = item {
                    if let Ok(lopdf::Object::Stream(ref s)) = doc.get_object(*id) {
                        if let Ok(b) = s.decompressed_content() {
                            all_bytes.extend_from_slice(&b);
                            all_bytes.push(b'\n');
                        }
                    }
                }
            }
        }
        _ => {}
    }

    let content = lopdf::content::Content::decode(&all_bytes).unwrap();
    let mut op_counts = std::collections::HashMap::new();
    for op in &content.operations {
        *op_counts.entry(op.operator.clone()).or_insert(0u32) += 1;
    }

    println!("\nContent stream operators:");
    let mut sorted: Vec<_> = op_counts.iter().collect();
    sorted.sort_by_key(|(k, _)| k.clone());
    for (op, count) in &sorted {
        println!("  {}: {}", op, count);
    }

    // Check: are text operators present?
    let tj_count = op_counts.get("Tj").unwrap_or(&0);
    let tJ_count = op_counts.get("TJ").unwrap_or(&0);
    let bt_count = op_counts.get("BT").unwrap_or(&0);
    println!("\nText: BT={}, Tj={}, TJ={}", bt_count, tj_count, tJ_count);
}

#[test]
fn test_3037_draw_commands() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => { println!("File not found"); return; }
    };

    let renderer = openaec_pdf_v1::PdfRenderer::new();
    let doc = renderer.load_document(&bytes).unwrap();
    let cmds = doc.extract_draw_commands(0).unwrap();
    let cmd_bytes = cmds.into_bytes();
    println!("Total draw command bytes: {}", cmd_bytes.len());

    // Count opcodes
    let mut pos = 16usize; // skip header
    let mut counts = std::collections::HashMap::new();
    while pos < cmd_bytes.len() {
        let op = cmd_bytes[pos];
        pos += 1;
        *counts.entry(op).or_insert(0u32) += 1;
        match op {
            0 | 1 => pos += 8,
            2 => pos += 24,
            3 => pos += 16,
            4 => {},
            5 => pos += 8,
            6 => pos += 4,
            7 | 8 | 9 | 10 | 11 => {},
            12 => pos += 24,
            13 | 14 => pos += 1,
            15 => pos += 4,
            16 => { let c = cmd_bytes[pos] as usize; pos += 1 + c * 4 + 4; },
            17 => {},
            18 => { pos += 16; let l = cmd_bytes[pos] as usize; pos += 1 + l; },
            19 => {
                pos += 4; // w, h
                let dl = u32::from_le_bytes([cmd_bytes[pos], cmd_bytes[pos+1], cmd_bytes[pos+2], cmd_bytes[pos+3]]) as usize;
                pos += 4 + dl;
            },
            20 | 21 => {},
            _ => { println!("Unknown opcode {} at {}", op, pos - 1); break; }
        }
    }

    let names = [
        (0, "MoveTo"), (1, "LineTo"), (2, "CubicTo"), (3, "Rect"), (4, "Close"),
        (5, "SetStroke"), (6, "SetFill"), (7, "Stroke"), (8, "Fill"), (9, "FillEvenOdd"),
        (10, "Save"), (11, "Restore"), (12, "Transform"), (17, "BeginPath"),
        (18, "TextAt"), (19, "DrawImage"), (20, "Clip"), (21, "ClipEvenOdd"),
    ];
    for (op, name) in &names {
        if let Some(count) = counts.get(op) {
            println!("  {}: {}", name, count);
        }
    }

    // Check: are there any glyph-related commands (MoveTo/LineTo/CubicTo inside Save/Restore with small transforms)?
    let has_text = counts.get(&0).unwrap_or(&0) > &10; // Many MoveTo = likely glyphs
    println!("\nHas text glyphs: {} (MoveTo count: {})", has_text, counts.get(&0).unwrap_or(&0));
}
