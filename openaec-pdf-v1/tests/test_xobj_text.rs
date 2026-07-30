use std::fs;

fn obj_f32(o: &lopdf::Object) -> f32 {
    match o {
        lopdf::Object::Real(r) => *r as f32,
        lopdf::Object::Integer(i) => *i as f32,
        _ => 0.0,
    }
}

#[test]
fn test_xobj_text_detail() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let doc = lopdf::Document::load_from(std::io::Cursor::new(&bytes)).unwrap();
    let pages = doc.get_pages();
    let mut sorted: Vec<_> = pages.iter().collect();
    sorted.sort_by_key(|(n, _)| *n);
    let (_, &page_id) = sorted[0];
    let page_obj = doc.get_object(page_id).unwrap();
    let dict = page_obj.as_dict().unwrap();
    let res = match dict.get(b"Resources").unwrap() {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        lopdf::Object::Dictionary(d) => d.clone(),
        _ => return,
    };
    let xobjs = match res.get(b"XObject").unwrap() {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        lopdf::Object::Dictionary(d) => d.clone(),
        _ => return,
    };

    let mut count = 0;
    for (name, xref) in xobjs.iter() {
        if let lopdf::Object::Reference(id) = xref {
            if let Ok(lopdf::Object::Stream(ref stream)) = doc.get_object(*id) {
                let st = stream.dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok()); if st != Some(b"Form") { continue; }
                if let Ok(cb) = stream.decompressed_content() {
                    if let Ok(content) = lopdf::content::Content::decode(&cb) {
                        let mut font = String::new();
                        let mut size = 0.0f32;
                        let mut tm = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
                        for op in &content.operations {
                            match op.operator.as_str() {
                                "Tf" if op.operands.len() >= 2 => {
                                    if let lopdf::Object::Name(ref n) = op.operands[0] { font = String::from_utf8_lossy(n).to_string(); }
                                    size = obj_f32(&op.operands[1]);
                                }
                                "Tm" if op.operands.len() >= 6 => {
                                    for i in 0..6 { tm[i] = obj_f32(&op.operands[i]); }
                                }
                                "Tj" | "TJ" if count < 15 => {
                                    let text: String = match op.operands.first() {
                                        Some(lopdf::Object::String(b, _)) => String::from_utf8_lossy(b).to_string(),
                                        Some(lopdf::Object::Array(arr)) => arr.iter().filter_map(|item| {
                                            if let lopdf::Object::String(b, _) = item { Some(String::from_utf8_lossy(b).to_string()) } else { None }
                                        }).collect(),
                                        _ => String::new(),
                                    };
                                    let xn = String::from_utf8_lossy(name);
                                    println!("XObj={} font={} size={:.1} tm=[{:.3},{:.3},{:.3},{:.3},{:.1},{:.1}] '{}'",
                                        xn, font, size, tm[0], tm[1], tm[2], tm[3], tm[4], tm[5], text);
                                    count += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    println!("Total: {}", count);
}
