//! Divergentie-detector: vergelijkt de oude content-decoder (lopdf, de
//! referentie van vÃ³Ã³r de streaming-lexer) op-voor-op met ContentStreamIter,
//! over alle pagina's van alle opgegeven PDF's. Elke afwijking (operator,
//! operand-aantal of numerieke waarde) is een potentiÃ«le regressie van de
//! lexer-omzetting en wordt met context geprint.
//!
//! Gebruik: lexer_diff <map-of-pdf> ...

use lopdf::content::{Content, Operation};
use lopdf::Object;
use openaec_pdf_v1::content_stream::ContentStreamIter;
use std::path::{Path, PathBuf};

fn collect(args: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for a in args {
        let p = Path::new(a);
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    let f = e.path();
                    if f.extension().map_or(false, |x| x.eq_ignore_ascii_case("pdf")) {
                        out.push(f);
                    }
                }
            }
        } else if p.is_file() {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out
}

fn obj_eq(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Integer(x), Object::Integer(y)) => x == y,
        (Object::Real(x), Object::Real(y)) => (x - y).abs() < 1e-4,
        (Object::Integer(x), Object::Real(y)) | (Object::Real(y), Object::Integer(x)) => {
            (*x as f32 - y).abs() < 1e-4
        }
        (Object::Name(x), Object::Name(y)) => x == y,
        (Object::String(x, _), Object::String(y, _)) => x == y,
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Null, Object::Null) => true,
        (Object::Array(x), Object::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| obj_eq(p, q))
        }
        (Object::Dictionary(x), Object::Dictionary(y)) => x.len() == y.len(),
        _ => false,
    }
}

fn fmt_op(op: &Operation) -> String {
    let args: Vec<String> = op.operands.iter().map(|o| format!("{:?}", o)).collect();
    format!("{} {}", args.join(" "), op.operator)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pdfs = collect(&args);
    println!("lexer-divergentie over {} bestanden\n", pdfs.len());
    let mut total_pages = 0usize;
    let mut diverged = 0usize;

    for pdf in &pdfs {
        let name = pdf.file_name().unwrap().to_string_lossy().to_string();
        let bytes = match std::fs::read(pdf) { Ok(b) => b, Err(_) => continue };
        let doc = match lopdf::Document::load_mem(&bytes) { Ok(d) => d, Err(e) => { println!("{name}: lopdf laad-fout: {e}"); continue; } };
        let pages = doc.get_pages();
        for (pno, page_id) in pages.iter() {
            // Content-stream(s) van de pagina samenvoegen zoals de parser doet.
            let content_bytes = match doc.get_page_content(*page_id) { Ok(c) => c, Err(_) => continue };
            total_pages += 1;

            let old = match Content::decode(&content_bytes) {
                Ok(c) => c.operations,
                Err(_) => {
                    // lopdf kan zelf al falen (bv. op inline images) â€” dan is er
                    // geen referentie; sla over maar meld het.
                    println!("{name} p{pno}: lopdf-decode faalt (geen referentie)");
                    continue;
                }
            };

            let mut it = ContentStreamIter::new(&content_bytes);
            let mut op = Operation { operator: String::new(), operands: Vec::new() };
            let mut new_ops: Vec<Operation> = Vec::new();
            while it.next_into(&mut op) {
                new_ops.push(Operation { operator: op.operator.clone(), operands: op.operands.clone() });
            }

            // lopdf produceert bij BI/ID/EI rommel-tokens; filter beide stromen op
            // ops die de interpreter kent zodat we ECHTE regressies vergelijken.
            let known = |o: &Operation| !matches!(o.operator.as_str(), "BI" | "ID" | "EI");
            let old_f: Vec<&Operation> = old.iter().filter(|o| known(o)).collect();
            let new_f: Vec<&Operation> = new_ops.iter().filter(|o| known(o)).collect();

            let mut mism: Option<(usize, String, String)> = None;
            let n = old_f.len().max(new_f.len());
            for i in 0..n {
                match (old_f.get(i), new_f.get(i)) {
                    (Some(a), Some(b)) => {
                        if a.operator != b.operator
                            || a.operands.len() != b.operands.len()
                            || !a.operands.iter().zip(&b.operands).all(|(p, q)| obj_eq(p, q))
                        {
                            mism = Some((i, fmt_op(a), fmt_op(b)));
                            break;
                        }
                    }
                    (Some(a), None) => { mism = Some((i, fmt_op(a), "<einde>".into())); break; }
                    (None, Some(b)) => { mism = Some((i, "<einde>".into(), fmt_op(b))); break; }
                    (None, None) => break,
                }
            }

            if let Some((i, a, b)) = mism {
                diverged += 1;
                println!("{name} p{pno}: DIVERGENTIE op op #{i} (oud {} ops, nieuw {} ops)", old_f.len(), new_f.len());
                println!("   oud:   {}", a.chars().take(160).collect::<String>());
                println!("   nieuw: {}", b.chars().take(160).collect::<String>());
                // context: de 2 ops ervÃ³Ã³r uit de oude stroom
                for j in i.saturating_sub(2)..i {
                    if let Some(o) = old_f.get(j) { println!("   ctx-{}: {}", i - j, fmt_op(o).chars().take(120).collect::<String>()); }
                }
            }
        }
    }
    println!("\n{total_pages} pagina's vergeleken, {diverged} met divergentie");
}
