# OpenAEC Crates Warehouse

Rust crates voor bouw-energie en warmteverliesberekeningen — geëxtraheerd uit
[open-heatloss-studio](https://github.com/OpenAEC-Foundation/open-heatloss-studio)
zodat ze hergebruikt kunnen worden in andere OpenAEC projecten.

**Versie:** 0.1.1 (mei 2026)
**Workspace:** 18 crates · Rust edition 2021 · MIT

---

## Overzicht

| # | Categorie | Crates |
|---|-----------|--------|
| 1 | ISSO 51:2023 warmteverlies | 3 |
| 2 | NTA 8800 energieprestatie | 14 |
| 3 | Gedeeld (cross-norm) | 1 |
|   | **Totaal** | **18** |

---

## ISSO 51:2023 — warmteverlies (3 crates)

Berekening van warmteverlies voor woningen volgens ISSO 51:2023.

| Crate | Beschrijving |
|-------|--------------|
| [`isso51-core`](isso51-core/) | Pure rekenkern voor warmteverliesberekening van woongebouwen — formules, tabellen, validatie en JSON I/O. Optionele Vabi-import via feature `vabi-import`. |
| [`isso51-api`](isso51-api/) | REST API (Axum) rond `isso51-core`, met multipart-upload, SQLite-persistentie en cloud-integratie. |
| [`isso51-ifcx`](isso51-ifcx/) | IFCX (IFC5 alpha JSON) lezer/schrijver met `isso51::` namespace — converteert IFCX → `isso51-core::Project`. |

---

## NTA 8800 — energieprestatie (14 crates)

Implementatie van de Nederlandse bepalingsmethode NTA 8800 voor energieprestatie van gebouwen.

### Fundament

| Crate | Beschrijving |
|-------|--------------|
| [`nta8800-model`](nta8800-model/) | Gedeelde domein-types: `Gebouw`, `Rekenzone`, `EnergiefunctieRuimte`, `ConstructionLayer`, `Climate`, `MonthlyProfile`. |
| [`nta8800-tables`](nta8800-tables/) | Normatieve default-tabellen — klimaatdata H.17, lambda/U/psi (bijlagen E/F/G/H/I/L), afronding bijlage X. |
| [`nta8800-geometry`](nta8800-geometry/) | Gebouwbegrenzing en bepaling van oppervlakten en lengtes. |

### Energiestromen

| Crate | Beschrijving |
|-------|--------------|
| [`nta8800-transmission`](nta8800-transmission/) | Hoofdstuk 8 — transmissie warmteverlies, maandmethode, lineaire/punt-bruggen. |
| [`nta8800-ventilation`](nta8800-ventilation/) | Hoofdstuk 9 — ventilatie energiegebruik, luchtstromen, infiltratie. |
| [`nta8800-demand`](nta8800-demand/) | Maandelijkse warmte- en koudebehoefte bepaling. |

### Installaties

| Crate | Beschrijving |
|-------|--------------|
| [`nta8800-heating`](nta8800-heating/) | Hoofdstuk 11 — verwarming: afgifte, distributie, opwekking, regeling. |
| [`nta8800-cooling`](nta8800-cooling/) | Hoofdstuk 10 — koeling + vereenvoudigde koelbehoefte woningen (TOjuli-opvolger, bijlage AA). |
| [`nta8800-dhw`](nta8800-dhw/) | Hoofdstuk 13 — warm tapwater energiegebruik. |
| [`nta8800-humidity`](nta8800-humidity/) | Bevochtiging en ontvochtiging. |
| [`nta8800-lighting`](nta8800-lighting/) | Hoofdstuk 12 — verlichting energiegebruik. |
| [`nta8800-automation`](nta8800-automation/) | Hoofdstuk 14 — gebouwautomatisering factor-correcties. |

### Productie en eindscore

| Crate | Beschrijving |
|-------|--------------|
| [`nta8800-pv`](nta8800-pv/) | Gebouwgebonden PV-productie en hernieuwbare bronnen. |
| [`nta8800-ep`](nta8800-ep/) | EP-score integratie, primair energiegebruik, beleidsfactoren (CO₂, energielabel). |

---

## Gedeeld (1 crate)

| Crate | Beschrijving |
|-------|--------------|
| [`openaec-project-shared`](openaec-project-shared/) | V2 multi-calc project schema (`shared + geometry + calcs[]`) — zie ADR-002. Maakt het mogelijk om één projectbestand met meerdere berekeningen (ISSO 51, NTA 8800, …) op te zetten. |

---

## Afhankelijkheidsgraaf

Ruwe hiërarchie binnen de NTA 8800 keten (lager hangt af van hoger):

```
nta8800-model
   ├── nta8800-tables
   │     ├── nta8800-geometry
   │     │     └── nta8800-transmission
   │     │              └── nta8800-demand
   │     │                     ├── nta8800-heating
   │     │                     └── nta8800-cooling
   │     ├── nta8800-ventilation ─► nta8800-demand
   │     ├── nta8800-dhw
   │     ├── nta8800-humidity
   │     ├── nta8800-lighting
   │     └── nta8800-pv
   ├── nta8800-automation
   └── nta8800-ep
```

ISSO 51 keten:

```
isso51-core ──► isso51-ifcx
            └── isso51-api  (hangt ook af van enkele NTA 8800 crates + openaec-cloud)
```

Cross-norm:

```
openaec-project-shared ─► isso51-core + nta8800-{model, tables, transmission,
                                                ventilation, demand, cooling}
```

---

## Gebruik

### Bouwen en testen

```bash
cargo check --workspace
cargo test  --workspace
```

### Als git-dependency in een ander project

```toml
[dependencies]
isso51-core            = { git = "https://github.com/OpenAEC-Foundation/crates-warehouse" }
nta8800-model          = { git = "https://github.com/OpenAEC-Foundation/crates-warehouse" }
openaec-project-shared = { git = "https://github.com/OpenAEC-Foundation/crates-warehouse" }
```

### Optionele features

| Crate | Feature | Effect |
|-------|---------|--------|
| `isso51-core` | `vabi-import` | Schakelt Vabi `.vabi`-bestand import in (vereist `rusqlite`, `zip`, `tempfile`). |

---

## Versie & licentie

- **Versie** 0.1.1 — uniform via `[workspace.package]` in de root-`Cargo.toml`.
- **Licentie** MIT — zie [`LICENSE`](LICENSE).

---

## Bron

Deze crates zijn een snapshot van `open-heatloss-studio/crates/*` (commit mei 2026).
Voor de actuele ontwikkeling, zie de
[bron-repo](https://github.com/OpenAEC-Foundation/open-heatloss-studio).
