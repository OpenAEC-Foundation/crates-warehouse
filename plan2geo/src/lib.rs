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

    // annotaties en omgeving eerst (anders vangen de disciplinetokens ze)
    if l.contains("DIMENSION") || l.contains("SLASH") || l.contains("MAATV") || l.contains("BEMATING") {
        return "maatvoering";
    }
    if l.contains("TOPOGRAPHY") || l.contains("TOPO")
        || heeft("GEBOUW") || heeft("PAND") || heeft("WEGKANT") || heeft("BOOM")
        || heeft("INRIT") || heeft("SLOOT") || heeft("HAAG") || heeft("HEK")
    {
        return "topografie";
    }
    // nutsbedrijf-conventies: G_SERVICE / G_30MB = gas-LD (aansluitingen, 30 mbar)
    if l.contains("G_SERVICE") || l.contains("30MB") || l.contains("G_100MB") {
        return "gasLageDruk";
    }

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

/// NLCS++-status afleiden uit de laagnaam (default: BESTAAND).
fn status_uit_laag(laag: &str) -> &'static str {
    let l = laag.to_uppercase();
    if l.contains("VERWIJDER") || l.contains("- VERW") || l.contains("VERVALLEN") {
        "VERWIJDERD"
    } else if l.contains("REVISIE") {
        "REVISIE"
    } else if l.contains("NIEUW") || l.contains("AAN TE LEGGEN") || l.contains("ONTWERP") {
        "NIEUW"
    } else {
        "BESTAAND"
    }
}

/// Discipline-groep uit de laag (voor de objecttype-keuze).
#[derive(Clone, Copy, PartialEq)]
enum Disc {
    Ms,
    Ls,
    Hs,
    Gas,
    Water,
    Telecom,
    Geen,
}

fn disc_uit_laag(l_upper: &str, tokens: &[&str]) -> Disc {
    let heeft = |t: &str| tokens.iter().any(|x| *x == t);
    if heeft("MS") || l_upper.contains("MIDDENSPANNING") {
        Disc::Ms
    } else if heeft("LS") || heeft("OVL") || l_upper.contains("LAAGSPANNING") {
        Disc::Ls
    } else if heeft("HS") || l_upper.contains("HOOGSPANNING") {
        Disc::Hs
    } else if heeft("HD") || heeft("LD") || heeft("GAS") {
        Disc::Gas
    } else if l_upper.contains("WATER") || heeft("W") || heeft("WL") {
        Disc::Water
    } else if l_upper.contains("GLASVEZEL") || l_upper.contains("DATA") || l_upper.contains("TELE") {
        Disc::Telecom
    } else {
        Disc::Geen
    }
}

/// NLCS++-objecttype afleiden uit laagnaam + entiteitsoort + blocknaam.
/// Alleen mappen wat de standaard kent; anders None (blijft CAD-element).
fn objecttype_uit_laag(laag: &str, is_lijn: bool, block: Option<&str>) -> Option<&'static str> {
    let l = laag.to_uppercase();
    let b = block.map(|s| s.to_uppercase()).unwrap_or_default();
    let samen = format!("{l} {b}");
    let tokens: Vec<&str> = l
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let disc = disc_uit_laag(&l, &tokens);

    // annotatie/omgeving krijgt nooit een objecttype
    if matches!(thema_uit_laag(laag), "maatvoering" | "topografie") {
        return None;
    }
    // nutsbedrijf-conventies: gas-aansluitleidingen en -aansluitpunten
    if samen.contains("SERVICE_PIPE") || samen.contains("SERVICE_CONNECTION") || samen.contains("30MB") {
        return Some(if is_lijn { "Gleiding" } else { "Goverdrachtspunt" });
    }
    // expliciete puntobjecten eerst
    if samen.contains("MANTELBUIS") {
        return Some("Amantelbuis");
    }
    if samen.contains("MOF") {
        return match disc {
            Disc::Ms => Some("MSmof"),
            Disc::Ls => Some("LSmof"),
            Disc::Hs => Some("HSmof"),
            Disc::Telecom => Some("Tmof"),
            _ => None,
        };
    }
    if samen.contains("STATION") {
        return match disc {
            Disc::Ms => Some("MSstation"),
            Disc::Hs => Some("HSstation"),
            Disc::Gas => Some("Gstation"),
            Disc::Telecom => Some("Tstation"),
            _ => None,
        };
    }
    if samen.contains("KAST") && disc == Disc::Ls {
        return Some("LSkast");
    }
    if samen.contains("OVERDRACHT") || samen.contains("HUISAANSL") {
        return match disc {
            Disc::Ms => Some("MSoverdrachtspunt"),
            Disc::Ls | Disc::Geen => Some("LSoverdrachtspunt"),
            Disc::Gas => Some("Goverdrachtspunt"),
            Disc::Telecom => Some("Toverdrachtspunt"),
            _ => None,
        };
    }
    if samen.contains("AARDPEN") {
        return Some("Eaardpen");
    }
    if samen.contains("AFSLUITER") && disc == Disc::Gas {
        return Some("Gafsluiter");
    }

    // lijnvormige netelementen
    if is_lijn {
        return match disc {
            Disc::Ms => Some("MSkabel"),
            Disc::Ls => Some("LSkabel"),
            Disc::Hs => Some("HSkabel"),
            Disc::Gas => Some("Gleiding"),
            Disc::Telecom => Some("Tkabel"),
            Disc::Water | Disc::Geen => None, // water valt buiten NLCS++ E/G/T
        };
    }
    None
}

/// Materiaal-token uit de laagnaam (PE, PVC, ST, CU, …) — voor mantelbuizen e.d.
fn materiaal_uit_laag(laag: &str) -> Option<&'static str> {
    let l = laag.to_uppercase();
    for m in ["PE", "PVC", "HDPE", "ST", "CU", "GY"] {
        if l.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| t == m) {
            return Some(match m {
                "PE" => "PE",
                "PVC" => "PVC",
                "HDPE" => "HDPE",
                "ST" => "ST",
                "CU" => "CU",
                _ => "GY",
            });
        }
    }
    None
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
    let mut objecttypen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut viewports: Vec<Value> = Vec::new();

    for ent in doc.entities() {
        stats.entiteiten += 1;
        // Paperspace-viewports: de exacte blad→model-transformatie (georeferentie
        // van de plot-PDF). Alleen viewports die een RD-modelgebied tonen.
        if let acadrust::entities::EntityType::Viewport(v) = ent {
            if v.width > 0.0 && v.height > 0.0 && v.view_height > 0.0 {
                // view_center is in DCS; bij een getwiste view geldt WCS = R(twist)·DCS.
                // We exporteren raw waarden; de afnemer bepaalt de transformatie.
                let schaal = v.view_height / v.height;
                viewports.push(json!({
                    "paperCenter": [round2(v.center.x), round2(v.center.y)],
                    "paperSize": [round2(v.width), round2(v.height)],
                    "viewCenterDcs": [round2(v.view_center.x), round2(v.view_center.y)],
                    "viewHeight": round2(v.view_height),
                    "twist": v.twist_angle,
                    "schaal": round2(schaal),
                }));
            }
            continue;
        }
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

        // CAD-entiteit → GIS-element: objecttype (NLCS++) + status uit de laag
        let is_lijn = geometry["type"] == "LineString" || geometry["type"] == "MultiLineString";
        let block = extra.get("block").and_then(|v| v.as_str()).map(String::from);
        let objecttype = objecttype_uit_laag(&laag, is_lijn, block.as_deref());
        let status = status_uit_laag(&laag);

        let mut kern = Map::new();
        kern.insert("thema".into(), json!(thema_uit_laag(&laag)));
        if let Some(ot) = objecttype {
            kern.insert("objecttype".into(), json!(ot));
            *objecttypen.entry(ot.to_string()).or_insert(0) += 1;
        } else {
            *objecttypen.entry(format!("(cad) {ent_type}")).or_insert(0) += 1;
        }
        kern.insert("status".into(), json!(status));
        kern.insert("type".into(), json!(ent_type));
        if let Some(m) = materiaal_uit_laag(&laag) {
            kern.insert("materiaal".into(), json!(m));
        }

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
    fc["baken"]["bron"]["objecttypen"] = json!(objecttypen);
    if !viewports.is_empty() {
        fc["baken"]["bron"]["viewports"] = json!(viewports);
    }
    let (pl_min, pl_max) = (doc.header.paper_space_limits_min, doc.header.paper_space_limits_max);
    if pl_max.x > pl_min.x && pl_max.y > pl_min.y {
        fc["baken"]["bron"]["paperLimits"] =
            json!([round2(pl_min.x), round2(pl_min.y), round2(pl_max.x), round2(pl_max.y)]);
    }
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
