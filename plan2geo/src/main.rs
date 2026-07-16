//! CLI: plan2geo <dwg|dxf> [--project slug] [-o out.geojson]

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut out: Option<PathBuf> = None;
    let mut project = String::from("onbekend");
    let mut bestand: Option<PathBuf> = None;

    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" | "--out" => out = args.next().map(PathBuf::from),
            "--project" => {
                if let Some(p) = args.next() {
                    project = p;
                }
            }
            "-h" | "--help" => {
                eprintln!("gebruik: plan2geo <dwg|dxf> [--project slug] [-o out.geojson]");
                return ExitCode::SUCCESS;
            }
            _ => bestand = Some(PathBuf::from(a)),
        }
    }

    let Some(pad) = bestand else {
        eprintln!("fout: geen DWG/DXF opgegeven");
        return ExitCode::FAILURE;
    };
    let bytes = match std::fs::read(&pad) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("fout: {}: {e}", pad.display());
            return ExitCode::FAILURE;
        }
    };
    let naam = pad.file_name().and_then(|n| n.to_str()).unwrap_or("bestand").to_string();

    match plan2geo::convert_cad_bytes(&bytes, &naam, &project) {
        Ok(fc) => {
            eprintln!(
                "  {} → {} features · samenvatting: {}",
                naam,
                fc["features"].as_array().map(|a| a.len()).unwrap_or(0),
                serde_json::to_string(&fc["baken"]["samenvatting"]).unwrap_or_default()
            );
            eprintln!(
                "  bron: {}",
                serde_json::to_string(&fc["baken"]["bron"]).unwrap_or_default()
            );
            let txt = serde_json::to_string(&fc).expect("serialisatie");
            match out {
                Some(p) => {
                    if let Some(dir) = p.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(e) = std::fs::write(&p, txt) {
                        eprintln!("fout bij schrijven {}: {e}", p.display());
                        return ExitCode::FAILURE;
                    }
                    eprintln!("  geschreven: {}", p.display());
                }
                None => println!("{txt}"),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fout: {e}");
            ExitCode::FAILURE
        }
    }
}
