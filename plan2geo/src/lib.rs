//! Plankaart (DWG/DXF) → Baken GeoJSON-profiel (laagtype `ontwerp`).
//!
//! Leest de CAD-geometrie via acadrust (de RUST-DWG-lijn: pure Rust,
//! DWG R13–R2018 + DXF), zet entiteiten om naar features en leidt het
//! PMKL-thema af uit de NLCS-laagnaam. Coördinaten buiten het RD-bereik
//! (titelblok, papierruimte) worden weggefilterd en geteld.

use baken_geo::{round2, Bron, Laagtype};
use serde_json::{json, Map, Value};
use std::io::Cursor;

/// RD-geldigheidsbereik (EPSG:28992).
fn in_rd(x: f64, y: f64) -> bool {
    (0.0..=300_000.0).contains(&x) && (300_000.0..=630_000.0).contains(&y)
}

/// PMKL-thema afleiden uit een CAD-laagnaam (NLCS-conventies én vrije
/// tekenkamer-namen zoals "ENEXIS MS NIEUW" of "MS Verbindingsmof").
/// Token-gebaseerd: de laagnaam wordt gesplitst op niet-alfanumerieke tekens.
fn thema_uit_laag(laag: &str) -> &'static str {
    let l = laag.to_uppercase();
    let tokens: Vec<&str> = l
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let heeft = |t: &str| tokens.iter().any(|x| *x == t);

    if heeft("MS") || l.contains("ET_MS") || l.contains("MIDDENSPANNING") {
        "middenspanning"
    } else if heeft("LS") || heeft("OVL") || l.contains("ET_LS") || l.contains("LAAGSPANNING") {
        "laagspanning"
    } else if heeft("HS") || l.contains("ET_HS") || l.contains("HOOGSPANNING") {
        "hoogspanning"
    } else if heeft("HD") || l.contains("GAS_HD") || l.contains("HOGEDRUK") {
        "gasHogeDruk"
    } else if heeft("LD") || l.contains("GAS_LD") || heeft("GAS") {
        "gasLageDruk"
    } else if l.contains("WATER") || heeft("W") || heeft("WL") {
        "water"
    } else if l.contains("DATA") || l.contains("TELE") || l.contains("GLASVEZEL") || heeft("GLAS") {
        "datatransport"
    } else if l.contains("RIOOL") {
        "rioolVrijverval"
    } else if l.contains("WARMTE") {
        "warmte"
    } else if l.contains("MANTELBUIS") {
        "mantelbuis"
    } else {
        "overig"
    }
}

fn seg_arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    // hoeken in radialen; segmenteer in ~24 stukken (NLCS kent geen bogen —
    // strooksgewijs benaderen zoals de standaard voorschrijft)
    let mut sweep = a1 - a0;
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let n = 24usize;
    (0..=n)
        .map(|i| {
            let a = a0 + sweep * (i as f64) / (n as f64);
            [round2(cx + r * a.cos()), round2(cy + r * a.sin())]
        })
        .collect()
}

struct Stats {
    entiteiten: usize,
    gefilterd: usize,
    tekst_overgeslagen: usize,
}

/// Converteert DWG- of DXF-bytes naar een profiel-FeatureCollection.
pub fn convert_cad_bytes(bytes: &[u8], bestandsnaam: &str, project: &str) -> Result<Value, String> {
    let is_dwg = bytes.len() > 6 && &bytes[0..4] == b"AC10";
    let doc = if is_dwg {
        acadrust::io::DwgReader::from_stream(Cursor::new(bytes.to_vec()))
            .read()
            .map_err(|e| format!("{bestandsnaam}: DWG: {e}"))?
    } else {
        acadrust::io::DxfReader::from_reader(Cursor::new(bytes.to_vec()))
            .map_err(|e| format!("{bestandsnaam}: DXF: {e}"))?
            .read()
            .map_err(|e| format!("{bestandsnaam}: DXF: {e}"))?
    };

    let mut features: Vec<Value> = Vec::new();
    let mut stats = Stats { entiteiten: 0, gefilterd: 0, tekst_overgeslagen: 0 };

    for ent in doc.entities() {
        stats.entiteiten += 1;
        let (geom, ent_type, laag, extra) = entity_geometry(ent);

        let Some(geometry) = geom else {
            if ent_type == "tekst" {
                stats.tekst_overgeslagen += 1;
            }
            continue;
        };

        // RD-filter: alles van de feature moet binnen RD vallen
        let mut ok = true;
        baken_geo::walk_coords(&geometry["coordinates"], &mut |x, y| {
            if !in_rd(x, y) {
                ok = false;
            }
        });
        if !ok {
            stats.gefilterd += 1;
            continue;
        }

        let mut kern = Map::new();
        kern.insert("thema".into(), json!(thema_uit_laag(&laag)));
        kern.insert("type".into(), json!(ent_type));
        kern.insert("status".into(), json!("BESTAAND"));

        let mut bron = extra;
        bron.insert("laag".into(), json!(laag));

        features.push(baken_geo::feature(kern, bron, geometry));
    }

    let bron = Bron {
        formaat: if is_dwg { "dwg".into() } else { "dxf".into() },
        bestanden: vec![bestandsnaam.to_string()],
        meldnummers: vec![],
    };
    let mut fc = baken_geo::envelope(
        &format!("ontwerp-{project}"),
        Laagtype::Ontwerp,
        project,
        &bron,
        features,
    );
    fc["baken"]["bron"]["entiteiten"] = json!(stats.entiteiten);
    fc["baken"]["bron"]["gefilterdBuitenRd"] = json!(stats.gefilterd);
    fc["baken"]["bron"]["tekstOvergeslagen"] = json!(stats.tekst_overgeslagen);
    Ok(fc)
}

/// Zet één entiteit om naar (geometrie, typelabel, laagnaam, bron-extra's).
fn entity_geometry(
    ent: &acadrust::entities::EntityType,
) -> (Option<Value>, &'static str, String, Map<String, Value>) {
    use acadrust::entities::EntityType as E;
    let mut extra = Map::new();
    match ent {
        E::Line(l) => {
            let c = json!([[round2(l.start.x), round2(l.start.y)], [round2(l.end.x), round2(l.end.y)]]);
            (Some(json!({"type":"LineString","coordinates":c})), "lijn", l.common.layer.clone(), extra)
        }
        E::LwPolyline(p) => {
            let pts: Vec<[f64; 2]> = p
                .vertices
                .iter()
                .map(|v| [round2(v.location.x), round2(v.location.y)])
                .collect();
            if pts.len() < 2 {
                return (None, "lijn", p.common.layer.clone(), extra);
            }
            (Some(json!({"type":"LineString","coordinates":pts})), "polylijn", p.common.layer.clone(), extra)
        }
        E::Polyline(p) => {
            let pts: Vec<[f64; 2]> = p
                .vertices
                .iter()
                .map(|v| [round2(v.location.x), round2(v.location.y)])
                .collect();
            if pts.len() < 2 {
                return (None, "lijn", p.common.layer.clone(), extra);
            }
            (Some(json!({"type":"LineString","coordinates":pts})), "polylijn", p.common.layer.clone(), extra)
        }
        E::Circle(c) => {
            extra.insert("straal".into(), json!(round2(c.radius)));
            (Some(json!({"type":"Point","coordinates":[round2(c.center.x), round2(c.center.y)]})), "cirkel", c.common.layer.clone(), extra)
        }
        E::Arc(a) => {
            let pts = seg_arc(a.center.x, a.center.y, a.radius, a.start_angle, a.end_angle);
            (Some(json!({"type":"LineString","coordinates":pts})), "boog", a.common.layer.clone(), extra)
        }
        E::Insert(i) => {
            extra.insert("block".into(), json!(i.block_name.clone()));
            (Some(json!({"type":"Point","coordinates":[round2(i.insert_point.x), round2(i.insert_point.y)]})), "block", i.common.layer.clone(), extra)
        }
        E::Point(p) => {
            (Some(json!({"type":"Point","coordinates":[round2(p.location.x), round2(p.location.y)]})), "punt", p.common.layer.clone(), extra)
        }
        E::Text(t) => (None, "tekst", t.common.layer.clone(), extra),
        E::MText(t) => (None, "tekst", t.common.layer.clone(), extra),
        _ => (None, "overig", String::new(), extra),
    }
}
