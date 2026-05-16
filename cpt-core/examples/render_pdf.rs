//! Generate a sample PDF for visual iteration.
//! Usage:  cargo run --example render_pdf -- <path_to_gef> <out.pdf>

use std::path::PathBuf;
use chrono::NaiveDate;
use cpt_core::{parse_auto, generate_single_cpt_pdf_bytes, ProjectMeta};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let gef_path = args.get(1).cloned().unwrap_or_else(||
        r"C:\Users\rickd\Documents\GitHub\verification-files\GEF-BRO-XML\2600356_01.GEF".into());
    let out_path = args.get(2).cloned().unwrap_or_else(||
        r"C:\Users\rickd\Documents\GitHub\cpt-viewer\.claude\worktrees\kind-black-926d95\test-output\latest.pdf".into());

    // Read with lossy UTF-8 fallback (some GEF files have Windows-1252 chars).
    let bytes = std::fs::read(&gef_path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();

    let cpt = parse_auto(&text)?;
    let project = ProjectMeta {
        title: "Test project".into(),
        client: "Jos Vrolijk Bouwbedrijf".into(),
        location: "Dordrecht, Haaswijkweg West 110".into(),
        project_number: "0673".into(),
        author: "Konings Grondboorbedrijf BV".into(),
        date: NaiveDate::from_ymd_opt(2025, 11, 10).unwrap(),
    };
    let pdf_bytes = generate_single_cpt_pdf_bytes(&cpt, &project);
    let out = PathBuf::from(&out_path);
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&out, pdf_bytes)?;
    println!("wrote {}", out.display());
    Ok(())
}
