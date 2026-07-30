use std::fs;

#[test]
fn test_count_text_commands() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let doc = lopdf::Document::load_from(std::io::Cursor::new(&bytes)).unwrap();
    let pages = doc.get_pages();
    let mut sorted: Vec<_> = pages.iter().collect();
    sorted.sort_by_key(|(n, _)| *n);
    let (_, &page_id) = sorted[0];
    
    let page_obj = doc.get_object(page_id).unwrap();
    let dict = page_obj.as_dict().unwrap();
    let contents_ref = dict.get(b"Contents").unwrap();
    
    let mut all_bytes = Vec::new();
    match contents_ref {
        lopdf::Object::Reference(id) => {
            let stream = doc.get_object(*id).unwrap();
            if let lopdf::Object::Stream(ref s) = *stream {
                all_bytes.extend(s.decompressed_content().unwrap());
            }
        }
        lopdf::Object::Array(arr) => {
            for item in arr {
                if let lopdf::Object::Reference(id) = item {
                    let stream = doc.get_object(*id).unwrap();
                    if let lopdf::Object::Stream(ref s) = *stream {
                        all_bytes.extend(s.decompressed_content().unwrap());
                    }
                }
            }
        }
        _ => {}
    }
    
    let content = lopdf::content::Content::decode(&all_bytes).unwrap();
    let mut bt_count = 0;
    let mut tj_count = 0;
    let mut do_count = 0;
    let mut path_count = 0;
    
    for op in &content.operations {
        match op.operator.as_str() {
            "BT" => bt_count += 1,
            "Tj" | "TJ" => tj_count += 1,
            "Do" => do_count += 1,
            "m" | "l" | "c" => path_count += 1,
            _ => {}
        }
    }
    
    println!("Content stream operators:");
    println!("  BT (begin text): {}", bt_count);
    println!("  Tj/TJ (show text): {}", tj_count);
    println!("  Do (XObject): {}", do_count);
    println!("  m/l/c (path): {}", path_count);
    println!("  Total ops: {}", content.operations.len());
}
