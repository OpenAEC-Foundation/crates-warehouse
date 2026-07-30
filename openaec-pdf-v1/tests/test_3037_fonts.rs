use lopdf::{Document, Object};
use std::collections::HashSet;

#[test]
fn test_3037_text_and_fonts() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => { println!("File not found, skipping"); return; }
    };
    let doc = Document::load_mem(&bytes).unwrap();

    let pages = doc.get_pages();
    let (&page_num, &page_id) = pages.iter().next().unwrap();
    println!("\n=== Page {} ===", page_num);

    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();

    // Get resources
    let resources = get_resources(&doc, page);
    if let Some(ref res) = resources {
        print_fonts(&doc, res, "Page");
    }

    // Parse content stream for text operators
    let content_bytes = get_page_content(&doc, page);
    if let Some(bytes) = content_bytes {
        let content = lopdf::content::Content::decode(&bytes).unwrap();
        let mut text_count = 0;
        let mut font_refs = HashSet::new();

        for op in &content.operations {
            match op.operator.as_str() {
                "Tj" | "TJ" => text_count += 1,
                "Tf" => {
                    if let Some(Object::Name(ref name)) = op.operands.first() {
                        font_refs.insert(String::from_utf8_lossy(name).to_string());
                    }
                }
                _ => {}
            }
        }

        println!("\nDirect text ops on page: {}", text_count);
        println!("Font references: {:?}", font_refs);

        // If no text on page, check Form XObjects
        if text_count == 0 {
            println!("\nNo direct text — checking Form XObjects...");
            if let Some(ref res) = resources {
                check_xobjects_for_text(&doc, res);
            }
        }
    }
}

fn check_xobjects_for_text(doc: &Document, resources: &lopdf::Dictionary) {
    let xobj_ref = match resources.get(b"XObject") {
        Ok(o) => o,
        _ => { println!("No XObject in resources"); return; }
    };
    let xobj_dict = match xobj_ref {
        Object::Dictionary(d) => d.clone(),
        Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };

    for (name, obj_ref) in xobj_dict.iter() {
        let name_str = String::from_utf8_lossy(name);
        let resolved_id = match obj_ref {
            Object::Reference(id) => *id,
            _ => continue,
        };
        let obj = match doc.get_object(resolved_id) {
            Ok(o) => o,
            _ => continue,
        };
        let stream = match obj {
            Object::Stream(ref s) => s,
            _ => continue,
        };
        let subtype = stream.dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok());

        if subtype == Some(b"Form" as &[u8]) {
            if let Ok(content_bytes) = stream.decompressed_content() {
                if let Ok(content) = lopdf::content::Content::decode(&content_bytes) {
                    let mut text_count = 0;
                    let mut font_refs = HashSet::new();
                    let mut nested_do = Vec::new();

                    for op in &content.operations {
                        match op.operator.as_str() {
                            "Tj" | "TJ" => text_count += 1,
                            "Tf" => {
                                if let Some(Object::Name(ref n)) = op.operands.first() {
                                    font_refs.insert(String::from_utf8_lossy(n).to_string());
                                }
                            }
                            "Do" => {
                                if let Some(Object::Name(ref n)) = op.operands.first() {
                                    nested_do.push(String::from_utf8_lossy(n).to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    if text_count > 0 || !nested_do.is_empty() {
                        println!("  Form '{}': {} text ops, fonts: {:?}, nested Do: {:?}",
                            name_str, text_count, font_refs, nested_do);

                        // Print fonts in this Form's resources
                        if let Some(form_res) = get_form_resources(&stream.dict, doc) {
                            print_fonts(doc, &form_res, &format!("    Form '{}'", name_str));
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

        println!("  {} Fonts ({}):", prefix, fonts_dict.len());
        for (name, font_ref) in fonts_dict.iter() {
            let font_name = String::from_utf8_lossy(name);
            if let Object::Reference(id) = font_ref {
                if let Ok(font_obj) = doc.get_object(*id) {
                    if let Ok(fd) = font_obj.as_dict() {
                        let subtype = fd.get(b"Subtype").ok().and_then(|s| s.as_name().ok()).unwrap_or(b"?");
                        let base = fd.get(b"BaseFont").ok().and_then(|s| s.as_name().ok()).unwrap_or(b"?");
                        let has_desc = fd.has(b"FontDescriptor");

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

                        // Check for DescendantFonts (Type0)
                        let has_descendants = fd.has(b"DescendantFonts");

                        println!("    {} = {} / {} / embedded: {} / descendants: {}",
                            font_name,
                            String::from_utf8_lossy(subtype),
                            String::from_utf8_lossy(base),
                            embedded,
                            has_descendants,
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
            } else { None }
        }
        Object::Array(arr) => {
            let mut all = Vec::new();
            for item in arr {
                if let Object::Reference(id) = item {
                    if let Ok(Object::Stream(ref s)) = doc.get_object(*id) {
                        if let Ok(bytes) = s.decompressed_content() {
                            all.extend_from_slice(&bytes);
                            all.push(b'\n');
                        }
                    }
                }
            }
            Some(all)
        }
        _ => None,
    }
}
