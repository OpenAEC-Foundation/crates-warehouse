use std::fs;

#[test]
fn test_text_operators_detail() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let doc = lopdf::Document::load_from(std::io::Cursor::new(&bytes)).unwrap();
    let pages = doc.get_pages();
    let mut sorted: Vec<_> = pages.iter().collect();
    sorted.sort_by_key(|(n, _)| *n);
    let (_, &page_id) = sorted[0];

    let page_obj = doc.get_object(page_id).unwrap();
    let dict = page_obj.as_dict().unwrap();

    // Get content stream
    let contents = dict.get(b"Contents").unwrap();
    let mut all_bytes = Vec::new();
    if let lopdf::Object::Reference(id) = contents {
        if let lopdf::Object::Stream(ref s) = *doc.get_object(*id).unwrap() {
            all_bytes = s.decompressed_content().unwrap();
        }
    }

    let content = lopdf::content::Content::decode(&all_bytes).unwrap();

    let mut in_text = false;
    let mut current_font = String::new();
    let mut font_size = 0.0f32;
    let mut tm = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut count = 0;

    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => { in_text = true; tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]; }
            "ET" => { in_text = false; }
            "Tf" => {
                if op.operands.len() >= 2 {
                    if let lopdf::Object::Name(ref name) = op.operands[0] {
                        current_font = String::from_utf8_lossy(name).to_string();
                    }
                    font_size = match &op.operands[1] {
                        lopdf::Object::Real(r) => *r as f32,
                        lopdf::Object::Integer(i) => *i as f32,
                        _ => 0.0,
                    };
                }
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    for i in 0..6 {
                        tm[i] = match &op.operands[i] {
                            lopdf::Object::Real(r) => *r as f32,
                            lopdf::Object::Integer(i) => *i as f32,
                            _ => 0.0,
                        };
                    }
                }
            }
            "Tj" => {
                if count < 10 {
                    if let Some(lopdf::Object::String(bytes, _)) = op.operands.first() {
                        let text = String::from_utf8_lossy(bytes);
                        println!("Tj: font={} size={:.1} tm=[{:.2},{:.2},{:.2},{:.2},{:.1},{:.1}] text='{}'",
                            current_font, font_size,
                            tm[0], tm[1], tm[2], tm[3], tm[4], tm[5],
                            text);
                        count += 1;
                    }
                }
            }
            "TJ" => {
                if count < 10 {
                    if let Some(lopdf::Object::Array(arr)) = op.operands.first() {
                        let mut text = String::new();
                        for item in arr {
                            if let lopdf::Object::String(bytes, _) = item {
                                text += &String::from_utf8_lossy(bytes);
                            }
                        }
                        if !text.is_empty() {
                            println!("TJ: font={} size={:.1} tm=[{:.2},{:.2},{:.2},{:.2},{:.1},{:.1}] text='{}'",
                                current_font, font_size,
                                tm[0], tm[1], tm[2], tm[3], tm[4], tm[5],
                                text);
                            count += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    println!("\nTotal text operations shown: {}", count);
}
