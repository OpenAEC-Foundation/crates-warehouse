# cpt-core

CPT (Cone Penetration Test) domain library for the OpenAEC ecosystem.

## Features
- GEF 1.x parser (Dutch Geotechnical Exchange Format)
- BRO-XML parser (Dutch Basisregistratie Ondergrond CPT_O / CPT_O_DP)
- Robertson 1990 SBT classification (9 zones)
- Layer detection (consecutive same-zone grouping with min-thickness merge)
- RD ↔ WGS84 coordinate transformation (Kadaster polynomial, sub-meter accuracy)
- SVG plot rendering (NEN-EN-ISO 22476-1 layout)
- Report builder producing `openaec_core::ReportData`

## Usage

```rust
use cpt_core::{parse_auto, build_report, ProjectMeta, render_cpt_svg};

let text = std::fs::read_to_string("sondering.gef")?;
let cpt = parse_auto(&text)?;

let project = ProjectMeta {
    title: "My project".into(),
    client: "ACME bv".into(),
    location: "Amsterdam".into(),
    project_number: "2026-001".into(),
    author: "Open GEO Studio".into(),
    date: chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
};
let report = build_report(&[cpt.clone()], &project);

// Hand off to openaec-engine for PDF rendering:
// let pdf_bytes = openaec_engine::generate_pdf_bytes(&report)?;
// std::fs::write("rapport.pdf", pdf_bytes)?;

// Or render the plot directly as SVG:
let svg = render_cpt_svg(&cpt);
std::fs::write("sondering.svg", svg)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Architecture

```
parse_auto(text)
  ├─ "#GEF…" prefix → gef::parse
  └─ "<?xml…" prefix → bro::parse
                       │
                       ▼
               Cpt { id, metadata, position, points }
                       │
                ┌──────┼──────┬──────────┬──────────────┐
                ▼      ▼      ▼          ▼              ▼
        robertson  layers  coords  render_cpt_svg  build_report
        (classify) (detect) (RD↔WGS) (NEN ISO plot) (→ openaec)
```

## Tests

Integration tests run against real CPT files in
`C:/Users/rickd/Documents/GitHub/verification-files/GEF-BRO-XML/`:
- `voorbeeld.gef`, `cpt_pygef.gef` — synthetic + Pygef-exported GEFs
- `2600356_01.GEF` through `2600356_06.GEF` — real-world consultancy project series
- `cpt_bro.xml` — BRO-exported XML (CPT_O document, 305 measurement points)

Run: `cargo test -p cpt-core`

## License
MIT
