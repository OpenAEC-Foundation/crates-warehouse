//! Diagnose: welke afbeeldingsbronnen heeft een pagina?
//! - Image-XObjects in /Resources (recursief door Form-XObjects): W/H/BPC/CS/Filter
//! - Inline images (BI ... ID ... EI) in de content-stream: dicts + aantallen
//! Gebruik: img_probe <pdf> [pagina-index]
use lopdf::{Dictionary, Document, Object};
use std::collections::HashSet;

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(o),
        _ => o,
    }
}

fn dict_of<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Dictionary> {
    match resolve(doc, o) {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn show_xobjects(doc: &Document, res: &Dictionary, depth: usize, seen: &mut HashSet<(u32, u16)>) {
    let Some(xd) = res.get(b"XObject").ok().and_then(|o| dict_of(doc, o)) else { return };
    for (name, val) in xd.iter() {
        let id = match val {
            Object::Reference(id) => *id,
            _ => continue,
        };
        if !seen.insert(id) {
            continue;
        }
        let Ok(Object::Stream(s)) = doc.get_object(id) else { continue };
        let d = &s.dict;
        let sub = d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default();
        let get_i = |k: &[u8]| d.get(k).ok().map(|o| match resolve(doc, o) { Object::Integer(i) => i.to_string(), other => format!("{:?}", other) }).unwrap_or_default();
        let filter = d.get(b"Filter").ok().map(|o| format!("{:?}", o)).unwrap_or_default();
        let cs = d.get(b"ColorSpace").ok().map(|o| format!("{:?}", resolve(doc, o))).unwrap_or_default();
        let mask = d.get(b"ImageMask").ok().map(|o| format!("{:?}", o)).unwrap_or_default();
        let smask = d.get(b"SMask").is_ok();
        println!(
            "{:indent$}[{}] /{} sub={} W={} H={} BPC={} filter={} CS={} mask={} smask={} len={}",
            "", format!("{:?}", id), String::from_utf8_lossy(name), sub,
            get_i(b"Width"), get_i(b"Height"), get_i(b"BitsPerComponent"),
            filter, cs.chars().take(90).collect::<String>(), mask, smask, s.content.len(),
            indent = depth * 2
        );
        if sub == "Form" {
            if let Some(fres) = d.get(b"Resources").ok().and_then(|o| dict_of(doc, o)) {
                show_xobjects(doc, fres, depth + 1, seen);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pdf = args.first().expect("gebruik: img_probe <pdf> [pagina]");
    let page_idx: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let doc = Document::load(pdf).expect("load");
    let pages = doc.get_pages();
    let page_id = *pages.values().nth(page_idx).expect("pagina");
    let page = doc.get_dictionary(page_id).expect("page dict");

    println!("== XObjects (recursief) ==");
    if let Some(res) = page.get(b"Resources").ok().and_then(|o| dict_of(&doc, o)) {
        let mut seen = HashSet::new();
        show_xobjects(&doc, res, 0, &mut seen);
    }

    println!("\n== Inline images in content-stream ==");
    let content = doc.get_page_content(page_id).expect("content");
    println!("content: {:.1} MB gedecomprimeerd", content.len() as f64 / 1048576.0);
    // Ruwe scan op "BI" op token-grens gevolgd door /-dict en ID
    let mut i = 0usize;
    let mut n_bi = 0u64;
    let mut samples: Vec<String> = Vec::new();
    let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0C' | b'\0');
    let is_delim = |b: u8| matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%');
    while i + 1 < content.len() {
        if content[i] == b'B' && content[i + 1] == b'I' {
            let before_ok = i == 0 || is_ws(content[i - 1]) || is_delim(content[i - 1]);
            let after_ok = content.get(i + 2).map_or(true, |&b| is_ws(b) || b == b'/');
            if before_ok && after_ok {
                n_bi += 1;
                if samples.len() < 6 {
                    // toon dict tot aan ID
                    let end = (i + 300).min(content.len());
                    if let Some(id_pos) = content[i..end].windows(2).position(|w| w == b"ID") {
                        samples.push(String::from_utf8_lossy(&content[i..i + id_pos + 2]).replace(['\n', '\r'], " "));
                    }
                }
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    println!("BI-count (ruwe scan): {}", n_bi);
    for s in samples {
        println!("  {}", s);
    }

    // == DrawImage-payloads in de display-list (magic-bytes per op 19) ==
    println!("\n== DrawImage-payloads in extract_draw_commands ==");
    let bytes = std::fs::read(pdf).expect("read");
    let hdoc = openaec_pdf_v1::DocumentHandle::load(&bytes).expect("load handle");
    let d = hdoc.extract_draw_commands(page_idx, 0).expect("extract").into_bytes();
    let mut pos = 16usize;
    let mut n_img = 0u64;
    while pos < d.len() {
        let op = d[pos];
        pos += 1;
        let need = match op {
            0 | 1 => 8,
            2 | 12 => 24,
            3 => 16,
            5 => 8,
            6 => 4,
            13 | 14 => 1,
            15 => 4,
            16 => 1 + d[pos] as usize * 4 + 4,
            18 => 12 + 4 + 1 + d[pos + 16] as usize,
            19 => {
                let w = u16::from_le_bytes(d[pos..pos + 2].try_into().unwrap());
                let h = u16::from_le_bytes(d[pos + 2..pos + 4].try_into().unwrap());
                let dlen = u32::from_le_bytes(d[pos + 4..pos + 8].try_into().unwrap()) as usize;
                let m = &d[pos + 8..pos + 8 + 8.min(dlen)];
                n_img += 1;
                if n_img <= 8 {
                    println!("  img {}x{} len={} magic={:02X?} ({})", w, h, dlen, &m[..4.min(m.len())],
                        if m.starts_with(b"RGBA") { "RGBA-raw" }
                        else if m.starts_with(&[0xFF, 0xD8]) { "JPEG" }
                        else if m.starts_with(&[0x89, b'P']) { "PNG" }
                        else { "?" });
                }
                8 + dlen
            }
            _ => 0,
        };
        pos += need;
    }
    println!("totaal DrawImage: {}", n_img);
}
