#[test]
fn test_3037_font_encoding() {
    let path = r"C:\3BM\50_projecten\3_3BM_bouwtechniek\3037 Aanbouw Herenweg 20 Moerkapelle\71_constructie_advies\3037-CP-21 Constructieoverzicht.pdf";
    let bytes = match std::fs::read(path) { Ok(b) => b, Err(_) => return };
    let doc = lopdf::Document::load_mem(&bytes).unwrap();

    let pages = doc.get_pages();
    let (_, &page_id) = pages.iter().next().unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();

    let res = match page.get(b"Resources") {
        Ok(lopdf::Object::Dictionary(d)) => d.clone(),
        Ok(lopdf::Object::Reference(id)) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };

    let fonts = match res.get(b"Font") {
        Ok(lopdf::Object::Dictionary(d)) => d.clone(),
        Ok(lopdf::Object::Reference(id)) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
        _ => return,
    };

    for (name, fref) in fonts.iter() {
        let fname = String::from_utf8_lossy(name);
        let fid = match fref { lopdf::Object::Reference(id) => *id, _ => continue };
        let fobj = doc.get_object(fid).unwrap();
        let fdict = fobj.as_dict().unwrap();

        println!("\n=== Font '{}' ===", fname);

        // Print ALL keys in the font dictionary
        for (key, _) in fdict.iter() {
            println!("  Key: {}", String::from_utf8_lossy(key));
        }

        // Check Encoding
        if let Ok(enc) = fdict.get(b"Encoding") {
            println!("  Encoding: {:?}", enc);
            match enc {
                lopdf::Object::Name(n) => println!("    = Name: {}", String::from_utf8_lossy(n)),
                lopdf::Object::Reference(id) => {
                    let enc_obj = doc.get_object(*id).unwrap();
                    println!("    = Ref -> {:?}", enc_obj);
                    if let Ok(enc_dict) = enc_obj.as_dict() {
                        if let Ok(diff) = enc_dict.get(b"Differences") {
                            if let Ok(arr) = diff.as_array() {
                                println!("    Differences ({} entries):", arr.len());
                                for (i, item) in arr.iter().enumerate().take(30) {
                                    match item {
                                        lopdf::Object::Integer(n) => print!("      code={} ", n),
                                        lopdf::Object::Name(n) => print!("/{}  ", String::from_utf8_lossy(n)),
                                        _ => print!("?  "),
                                    }
                                    if i % 10 == 9 { println!(); }
                                }
                                println!();
                            }
                        }
                    }
                }
                lopdf::Object::Dictionary(d) => {
                    println!("    = Dict");
                    if let Ok(diff) = d.get(b"Differences") {
                        if let Ok(arr) = diff.as_array() {
                            println!("    Differences ({} entries):", arr.len());
                            for (i, item) in arr.iter().enumerate().take(30) {
                                match item {
                                    lopdf::Object::Integer(n) => print!("      code={} ", n),
                                    lopdf::Object::Name(n) => print!("/{}  ", String::from_utf8_lossy(n)),
                                    _ => print!("?  "),
                                }
                                if i % 10 == 9 { println!(); }
                            }
                            println!();
                        }
                    }
                }
                _ => {}
            }
        } else {
            println!("  NO Encoding entry");
        }
    }
}
