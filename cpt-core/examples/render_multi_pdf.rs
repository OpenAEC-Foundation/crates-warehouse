//! Generate a multi-CPT PDF (cover + table + per-CPT pages) using the
//! openaec engine path. Useful for sanity-checking that the original report
//! flow still works after the single-CPT direct-PDF additions.

use std::path::PathBuf;
use chrono::NaiveDate;
use cpt_core::{parse_auto, build_report, ProjectMeta};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new(r"C:\Users\rickd\Documents\GitHub\verification-files\GEF-BRO-XML");
    let mut cpts = Vec::new();
    for n in 1..=3 {
        let path = dir.join(format!("2600356_0{}.GEF", n));
        let bytes = std::fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        cpts.push(parse_auto(&text)?);
    }
    let project = ProjectMeta {
        title: "Multi-CPT test".into(),
        client: "Jos Vrolijk Bouwbedrijf".into(),
        location: "Dordrecht".into(),
        project_number: "0673".into(),
        author: "Konings Grondboorbedrijf BV".into(),
        date: NaiveDate::from_ymd_opt(2025, 11, 10).unwrap(),
    };
    let report = build_report(&cpts, &project);
    let pdf_bytes = openaec_core::generate_pdf_bytes(&report)?;
    let out = PathBuf::from(
        r"C:\Users\rickd\Documents\GitHub\cpt-viewer\.claude\worktrees\kind-black-926d95\test-output\multi.pdf"
    );
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&out, pdf_bytes)?;
    println!("wrote {}", out.display());
    Ok(())
}
