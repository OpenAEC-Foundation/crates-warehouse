# OpenAEC Crates Warehouse

Rust crates voor bouw-energie en warmteverlies berekeningen — geëxtraheerd uit
[open-heatloss-studio](https://github.com/OpenAEC-Foundation/open-heatloss-studio)
zodat ze hergebruikt kunnen worden in andere OpenAEC projecten.

**Versie:** 0.1.1 (mei 2026)

## Inhoud

### ISSO 51:2023 warmteverlies (3 crates)
- `isso51-core` — Pure rekenkern: formules, tabellen, validatie, JSON I/O
- `isso51-api` — Axum REST API rond `isso51-core`
- `isso51-ifcx` — IFCX (IFC5 alpha) parser → `isso51-core` Project

### NTA 8800 energieprestatie (14 crates)
- `nta8800-model` — Gedeelde domein-types
- `nta8800-tables` — Opzoektabellen (klimaat, factoren, ...)
- `nta8800-geometry` — Geometrie + transmissie-oppervlakken
- `nta8800-transmission` — Hoofdstuk 8 transmissie
- `nta8800-ventilation` — Hoofdstuk 9 ventilatie + infiltratie
- `nta8800-demand` — Energiebehoefte verwarming
- `nta8800-heating` — Hoofdstuk 11 verwarmingssystemen
- `nta8800-cooling` — Hoofdstuk 10 koeling (TO-juli, bijlage AA)
- `nta8800-dhw` — Hoofdstuk 13 warmtapwater
- `nta8800-ep` — EP-score, CO₂-factoren, energielabel
- `nta8800-automation` — Hoofdstuk 14 gebouwautomatisering
- `nta8800-humidity` — Vocht / luchtbehandeling
- `nta8800-lighting` — Hoofdstuk 12 verlichting
- `nta8800-pv` — PV-opbrengst

### Gedeeld (1 crate)
- `openaec-project-shared` — V2 multi-calc project schema (ADR-002)

## Gebruik

```bash
cargo check --workspace
cargo test --workspace
```

Vanuit een ander project:
```toml
[dependencies]
isso51-core = { git = "https://github.com/OpenAEC-Foundation/crates-warehouse" }
```

## Licentie

MIT — zie `LICENSE`.

## Bron

Deze crates zijn een snapshot van
`open-heatloss-studio/crates/*` (commit datum mei 2026).
Voor de actuele ontwikkeling, zie de bron-repo.
