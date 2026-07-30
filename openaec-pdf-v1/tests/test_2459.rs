use std::fs;

#[test]
fn test_2459_page_info() {
    let path = r"C:\Users\rickd\Documents\GitHub\open-pdf-studio\2459-TO_Fragmenten.pdf";
    let bytes = fs::read(path).unwrap();
    let doc = lopdf::Document::load_from(std::io::Cursor::new(&bytes)).unwrap();
    let pages = doc.get_pages();
    let mut sorted: Vec<_> = pages.iter().collect();
    sorted.sort_by_key(|(n, _)| *n);
    
    let (_, &page_id) = sorted[0];
    let page_obj = doc.get_object(page_id).unwrap();
    let dict = page_obj.as_dict().unwrap();
    
    println!("Page 1 dictionary keys:");
    for (key, val) in dict.iter() {
        let key_str = String::from_utf8_lossy(key);
        match val {
            lopdf::Object::Array(arr) if arr.len() == 4 => {
                println!("  {} = {:?}", key_str, arr);
            }
            _ => println!("  {} = {:?}", key_str, val),
        }
    }
    
    let boxes: Vec<&[u8]> = vec![b"MediaBox", b"CropBox", b"TrimBox", b"BleedBox", b"ArtBox"];
    for box_name in boxes {
        match dict.get(box_name) {
            Ok(val) => println!("\n{}: {:?}", String::from_utf8_lossy(box_name), val),
            Err(_) => println!("\n{}: NOT PRESENT", String::from_utf8_lossy(box_name)),
        }
    }
}
