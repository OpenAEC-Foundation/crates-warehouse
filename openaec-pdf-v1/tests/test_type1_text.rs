// Verifies Type1 fonts (e.g. UniviaPro) emit glyph outlines via system-font fallback.
use openaec_pdf_v1::PdfRenderer;
use std::fs;

fn run(path: &str) -> (usize, usize) {
    let bytes = fs::read(path).expect("read pdf");
    let r = PdfRenderer::new();
    let doc = r.load_document(&bytes).expect("load");
    let buf = doc.extract_draw_commands(0, 0).expect("extract");
    let raw = buf.into_bytes();
    let mut fills = 0usize;
    let mut transforms = 0usize;
    // Header: f32 x0, y0, w, h = 16 bytes
    let mut i = 16usize;
    while i < raw.len() {
        let op = raw[i]; i += 1;
        match op {
            0 | 1 => i += 8,            // MoveTo / LineTo
            2 => i += 24,                // CubicTo
            3 => i += 16,                // Rect
            4 => {}                      // ClosePath
            5 => i += 8,                 // SetStroke (rgba+width)
            6 => i += 4,                 // SetFill
            7 => {}                      // Stroke
            8 => fills += 1,             // Fill
            9 => {}                      // FillEvenOdd
            10 | 11 => {}                // Save/Restore
            12 => { transforms += 1; i += 24; }
            13 | 14 => i += 1,
            15 => i += 4,
            16 => {
                if i >= raw.len() { break; }
                let n = raw[i] as usize; i += 1;
                i += n * 4 + 4;
            }
            17 => {}
            18 => {
                if i + 17 > raw.len() { break; }
                i += 16;
                let len = raw[i] as usize; i += 1;
                i += len;
            }
            19 => {
                // DrawImage: u16 w + u16 h + u32 dataLen + bytes
                if i + 8 > raw.len() { break; }
                i += 4;
                let len = u32::from_le_bytes([raw[i],raw[i+1],raw[i+2],raw[i+3]]) as usize;
                i += 4 + len;
            }
            20 | 21 => {}                 // Clip / ClipEvenOdd
            _ => { eprintln!("unknown opcode {} at {}", op, i - 1); break; }
        }
    }
    (fills, transforms)
}

#[test]
fn ac294_emits_glyph_fills() {
    let (fills, transforms) = run("../test pdf-bestanden/AC294_offerte_stalen_bak_Vincent_Christe.pdf");
    eprintln!("AC294 page0: fills={} transforms={}", fills, transforms);
    assert!(fills > 100, "AC294 should emit many glyph fills, got {}", fills);
}

#[test]
fn vloerverwarming_still_renders() {
    let (fills, transforms) = run("../test pdf-bestanden/Vloerverwarming Woning Bert van Dorp - opm BvD.pdf");
    eprintln!("Vloerverwarming page0: fills={} transforms={}", fills, transforms);
    assert!(fills > 50, "Vloerverwarming regression: only {} fills", fills);
}
