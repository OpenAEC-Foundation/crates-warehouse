//! Telt opcodes in de display-list van MV-03 p1 â€” beslist of glyph-outlines
//! (save/transform/begin/pad/fill/restore-runs) en images in de buffer zitten.
fn main() {
    let pdf = "C:/Users/rickd/Documents/GitHub/verification-files/PDF-bestanden/MV-03_Mechanische ventilatie, 3e verdieping ontwerp ACH van 1,5 naar 2,0.pdf";
    let bytes = std::fs::read(pdf).expect("read");
    let doc = openaec_pdf_v1::DocumentHandle::load(&bytes).expect("load");
    let buf = doc.extract_draw_commands(0, 0).expect("extract").into_bytes();
    let mut counts = [0u64; 22];
    let mut pos = 16usize;
    let d = &buf;
    while pos < d.len() {
        let op = d[pos];
        pos += 1;
        if (op as usize) < counts.len() {
            counts[op as usize] += 1;
        } else {
            println!("ONBEKENDE opcode {} op {}", op, pos - 1);
            break;
        }
        pos += match op {
            0 | 1 => 8,
            2 | 12 => 24,
            3 => 16,
            5 => 8,
            6 => 4,
            13 | 14 => 1,
            15 => 4,
            16 => {
                let n = d[pos] as usize;
                1 + n * 4 + 4
            }
            18 => {
                let len = d[pos + 12 + 4] as usize;
                12 + 4 + 1 + len
            }
            19 => {
                let dlen = u32::from_le_bytes(d[pos + 4..pos + 8].try_into().unwrap()) as usize;
                8 + dlen
            }
            _ => 0,
        };
    }
    let names = ["MoveTo", "LineTo", "CubicTo", "Rect", "Close", "SetStroke", "SetFill", "Stroke", "Fill", "FillEO", "Save", "Restore", "Transform", "Cap", "Join", "Miter", "Dash", "BeginPath", "TextAt", "DrawImage", "Clip", "ClipEO"];
    for (i, n) in counts.iter().enumerate() {
        if *n > 0 {
            println!("{:>10}  {}", n, names[i]);
        }
    }
}
