//! CLI: klic2geo [-o out.geojson] [--name <laagnaam>] <klic-zip>...
//!
//! Converteert één of meer KLIC-leveringszips naar één samengevoegde
//! GeoJSON FeatureCollection (RD/EPSG:28992). Zonder -o naar stdout.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let mut out: Option<PathBuf> = None;
    let mut name = String::from("klic");
    let mut project = String::from("onbekend");
    let mut zips: Vec<PathBuf> = Vec::new();

    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" | "--out" => {
                out = args.next().map(PathBuf::from);
            }
            "--name" => {
                if let Some(n) = args.next() {
                    name = n;
                }
            }
            "--project" => {
                if let Some(p) = args.next() {
                    project = p;
                }
            }
            "-h" | "--help" => {
                eprintln!("gebruik: klic2geo [-o out.geojson] [--name laagnaam] [--project slug] <klic-zip>...");
                return ExitCode::SUCCESS;
            }
            _ => zips.push(PathBuf::from(a)),
        }
    }

    if zips.is_empty() {
        eprintln!("fout: geen KLIC-zips opgegeven (gebruik --help)");
        return ExitCode::FAILURE;
    }

    let mut leveringen = Vec::new();
    for z in &zips {
        match klic2geo::convert_zip(z) {
            Ok(lev) => {
                eprintln!(
                    "  {} → melding {} · {} features",
                    z.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                    lev.meldnummer,
                    lev.features.len()
                );
                leveringen.push(lev);
            }
            Err(e) => {
                eprintln!("fout: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    let bestanden: Vec<String> = zips
        .iter()
        .filter_map(|z| z.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();
    let fc = klic2geo::feature_collection(&name, &project, &bestanden, &leveringen);
    eprintln!(
        "  samenvatting: {}",
        serde_json::to_string(&fc["baken"]["samenvatting"]).unwrap_or_default()
    );

    let jsontxt = serde_json::to_string(&fc).expect("serialisatie");
    match out {
        Some(p) => {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(&p, jsontxt) {
                eprintln!("fout bij schrijven {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
            eprintln!("  geschreven: {}", p.display());
        }
        None => println!("{jsontxt}"),
    }
    ExitCode::SUCCESS
}
