//! Render the CPT page as PNG for visual verification (no PDF needed).
//! Usage:  cargo run --example render_png -- <path_to_gef> <out.png>

use std::path::PathBuf;
use cpt_core::{parse_auto, plot::render_cpt_png};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let gef_path = args.get(1).cloned().unwrap_or_else(||
        r"C:\Users\rickd\Documents\GitHub\verification-files\GEF-BRO-XML\2600356_01.GEF".into());
    let out_path = args.get(2).cloned().unwrap_or_else(||
        r"C:\Users\rickd\Documents\GitHub\cpt-viewer\.claude\worktrees\kind-black-926d95\test-output\sbt_check.png".into());

    let bytes = std::fs::read(&gef_path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let cpt = parse_auto(&text)?;
    let png = render_cpt_png(&cpt);
    let out = PathBuf::from(&out_path);
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&out, png)?;
    println!("wrote {}", out.display());
    Ok(())
}
