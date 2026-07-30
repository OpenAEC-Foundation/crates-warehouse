use lopdf::{Document, Object};
use std::collections::HashSet;

#[test]
fn test_2459_fonts_and_text() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = std::fs::read(path).unwrap();
    let doc = Document::load_mem(&bytes).unwrap();

    let pages = doc.get_pages();
    let (&page_num, &page_id) = pages.iter().next().unwrap();
    println!("\n=== Page {} (id: {:?}) ===", page_num, page_id);

    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();

    // Get MediaBox
    if let Ok(mb) = page.get(b"MediaBox") {
        println!("MediaBox: {:?}", mb);
    }

    // Get page resources
    let resources = get_resources(&doc, page);
    if let Some(ref res) = resources {
        print_fonts(&doc, res, "Page");
    }

    // Parse content stream to find Do operators (Form XObjects)
    let content_bytes = get_page_content(&doc, page);
    if let Some(bytes) = content_bytes {
        let content = lopdf::content::Content::decode(&bytes).unwrap();
        let mut xobject_names = Vec::new();
        let mut text_ops = Vec::new();

        for op in &content.operations {
            match op.operator.as_str() {
                "Do" => {
                    if let Some(Object::Name(name)) = op.operands.first() {
                        xobject_names.push(String::from_utf8_lossy(name).to_string());
                    }
                }
                "BT" | "ET" | "Tj" | "TJ" | "Td" | "TD" | "Tm" | "Tf" | "T*" => {
                    text_ops.push(format!("{} {:?}", op.operator, op.operands));
                }
                _ => {}
            }
        }

        println!("\nDirect text operators on page: {}", text_ops.len());
        for op in text_ops.iter().take(20) {
            println!("  {}", op);
        }

        println!("\nForm XObjects referenced: {:?}", xobject_names);

        // Examine each Form XObject
        if let Some(ref res) = resources {
            if let Ok(xobj_ref) = res.get(b"XObject") {
                let xobj_dict = match xobj_ref {
                    Object::Dictionary(d) => d.clone(),
                    Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
                    _ => return,
                };

                let mut examined = HashSet::new();
                for name in &xobject_names {
                    if examined.contains(name) { continue; }
                    examined.insert(name.clone());
                    examine_form_xobject(&doc, &xobj_dict, name);
                }
            }
        }
    }
}

fn examine_form_xobject(doc: &Document, xobj_dict: &lopdf::Dictionary, name: &str) {
    let obj_ref = match xobj_dict.get(name.as_bytes()) {
        Ok(o) => o,
        _ => return,
    };
    let resolved_id = match obj_ref {
        Object::Reference(id) => *id,
        _ => return,
    };
    let obj = match doc.get_object(resolved_id) {
        Ok(o) => o,
        _ => return,
    };
    let stream = match obj {
        Object::Stream(ref s) => s,
        _ => return,
    };
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok());
    if subtype != Some(b"Form" as &[u8]) {
        println!("\n  {} is NOT a Form XObject (subtype: {:?})", name, subtype);
        return;
    }

    let matrix = stream.dict.get(b"Matrix").ok().and_then(|m| m.as_array().ok());
    let bbox = stream.dict.get(b"BBox").ok().and_then(|m| m.as_array().ok());

    println!("\n  Form XObject '{}' (id: {:?}):", name, resolved_id);
    if let Some(m) = matrix {
        println!("    Matrix: {:?}", m);
    } else {
        println!("    Matrix: (none/identity)");
    }
    if let Some(b) = bbox {
        println!("    BBox: {:?}", b);
    }

    // Check fonts in this Form XObject
    if let Some(form_res) = get_form_resources(&stream.dict, doc) {
        print_fonts(doc, &form_res, &format!("  Form '{}'", name));
    }

    // Scan content for text operators
    if let Ok(content_bytes) = stream.decompressed_content() {
        if let Ok(content) = lopdf::content::Content::decode(&content_bytes) {
            let mut text_count = 0;
            let mut do_count = 0;
            let mut nested_xobjects = Vec::new();

            for op in &content.operations {
                match op.operator.as_str() {
                    "Tj" | "TJ" => text_count += 1,
                    "Do" => {
                        do_count += 1;
                        if let Some(Object::Name(n)) = op.operands.first() {
                            nested_xobjects.push(String::from_utf8_lossy(n).to_string());
                        }
                    }
                    _ => {}
                }
            }
            println!("    Text ops (Tj/TJ): {}, nested Do: {}", text_count, do_count);
            if !nested_xobjects.is_empty() {
                println!("    Nested XObjects: {:?}", nested_xobjects);

                // Examine nested Form XObjects
                if let Some(form_res) = get_form_resources(&stream.dict, doc) {
                    if let Ok(nested_xobj_ref) = form_res.get(b"XObject") {
                        let nested_xobj_dict = match nested_xobj_ref {
                            Object::Dictionary(d) => d.clone(),
                            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
                            _ => return,
                        };
                        for nested_name in nested_xobjects.iter().take(5) {
                            examine_form_xobject(doc, &nested_xobj_dict, nested_name);
                        }
                    }
                }
            }
        }
    }
}

fn print_fonts(doc: &Document, resources: &lopdf::Dictionary, prefix: &str) {
    if let Ok(fonts_obj) = resources.get(b"Font") {
        let fonts_dict = match fonts_obj {
            Object::Dictionary(d) => d.clone(),
            Object::Reference(id) => {
                if let Ok(obj) = doc.get_object(*id) {
                    if let Ok(d) = obj.as_dict() { d.clone() } else { return; }
                } else { return; }
            }
            _ => return,
        };

        println!("\n  {} Fonts ({}):", prefix, fonts_dict.len());
        for (name, font_ref) in fonts_dict.iter() {
            let font_name = String::from_utf8_lossy(name);
            if let Object::Reference(id) = font_ref {
                if let Ok(font_obj) = doc.get_object(*id) {
                    if let Ok(fd) = font_obj.as_dict() {
                        let subtype = fd.get(b"Subtype").ok().and_then(|s| s.as_name().ok()).unwrap_or(b"?");
                        let base = fd.get(b"BaseFont").ok().and_then(|s| s.as_name().ok()).unwrap_or(b"?");
                        let has_desc = fd.has(b"FontDescriptor");
                        let has_enc = fd.has(b"Encoding");

                        let mut embedded = "none";
                        if has_desc {
                            if let Ok(Object::Reference(did)) = fd.get(b"FontDescriptor") {
                                if let Ok(desc) = doc.get_object(*did) {
                                    if let Ok(dd) = desc.as_dict() {
                                        if dd.has(b"FontFile2") { embedded = "FontFile2 (TrueType)"; }
                                        else if dd.has(b"FontFile3") { embedded = "FontFile3 (CFF)"; }
                                        else if dd.has(b"FontFile") { embedded = "FontFile (Type1)"; }
                                    }
                                }
                            }
                        }

                        println!("    {} = {} / {} / embedded: {} / encoding: {}",
                            font_name,
                            String::from_utf8_lossy(subtype),
                            String::from_utf8_lossy(base),
                            embedded,
                            has_enc,
                        );
                    }
                }
            }
        }
    }
}

fn get_resources(doc: &Document, page: &lopdf::Dictionary) -> Option<lopdf::Dictionary> {
    let res = page.get(b"Resources").ok()?;
    match res {
        Object::Dictionary(d) => Some(d.clone()),
        Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok().cloned(),
        _ => None,
    }
}

fn get_form_resources(dict: &lopdf::Dictionary, doc: &Document) -> Option<lopdf::Dictionary> {
    let res = dict.get(b"Resources").ok()?;
    match res {
        Object::Dictionary(d) => Some(d.clone()),
        Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok().cloned(),
        _ => None,
    }
}

fn get_page_content(doc: &Document, page: &lopdf::Dictionary) -> Option<Vec<u8>> {
    let contents = page.get(b"Contents").ok()?;
    match contents {
        Object::Reference(id) => {
            if let Ok(Object::Stream(ref s)) = doc.get_object(*id) {
                s.decompressed_content().ok()
            } else {
                None
            }
        }
        Object::Array(arr) => {
            let mut all_bytes = Vec::new();
            for item in arr {
                if let Object::Reference(id) = item {
                    if let Ok(Object::Stream(ref s)) = doc.get_object(*id) {
                        if let Ok(bytes) = s.decompressed_content() {
                            all_bytes.extend_from_slice(&bytes);
                            all_bytes.push(b'\n');
                        }
                    }
                }
            }
            Some(all_bytes)
        }
        _ => None,
    }
}
