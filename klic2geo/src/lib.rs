//! KLIC (WIBON gebiedsinformatielevering) → GeoJSON.
//!
//! Leest de `GI_gebiedsinformatielevering_*_V2.xml` (IMKL 2.0 GML, EPSG:28992)
//! uit een Kadaster-leveringszip en zet de netelementen om naar GeoJSON:
//! - lijnelementen: Elektriciteitskabel, Waterleiding, Telecommunicatiekabel,
//!   OlieGasChemicalienPijpleiding, ThermischePijpleiding, Rioolleiding,
//!   Kabelbed, Mantelbuis (geometrie direct via `imkl:ligging` of indirect via
//!   `net:link` → `us-net-common:UtilityLink`)
//! - puntelementen: Appurtenance
//! - de Graafpolygoon (thema "graafpolygoon")
//! - netbeheerders via `imkl:Beheerder` (bronhoudercode KLxxxx → organisatienaam);
//!   de KL-code van een element wordt uit zijn gml:id herleid.

use baken_geo::{round2 as bg_round2, Bron, Laagtype};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

const LIJN_ELEMENTEN: [&str; 8] = [
    "Elektriciteitskabel",
    "Waterleiding",
    "Telecommunicatiekabel",
    "OlieGasChemicalienPijpleiding",
    "ThermischePijpleiding",
    "Rioolleiding",
    "Kabelbed",
    "Mantelbuis",
];

const GEO_CONTAINERS: [&str; 4] = ["centrelineGeometry", "geometry", "ligging", "geometrie2D"];

#[derive(Default, Debug)]
struct Element {
    kind: String,
    gml_id: String,
    thema: Option<String>,
    in_network: Vec<String>,
    links: Vec<String>,
    lines: Vec<Vec<[f64; 2]>>,
    point: Option<[f64; 2]>,
    ring: Option<Vec<[f64; 2]>>,
    diameter: Option<String>,
    materiaal: Option<String>,
    app_type: Option<String>,
    vertical: Option<String>,
    bronhoudercode: Option<String>,
    organisatie: Option<String>,
}

#[derive(Default)]
pub struct Levering {
    pub meldnummer: String,
    pub features: Vec<Value>,
}

fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit(':').next() {
        Some(l) => l.to_string(),
        None => s.to_string(),
    }
}

fn href_segment(href: &str) -> String {
    href.trim_start_matches('#')
        .rsplit('/')
        .next()
        .unwrap_or(href)
        .to_string()
}

fn attr_value(e: &quick_xml::events::BytesStart, key_suffix: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
        if k == key_suffix || k.ends_with(&format!(":{key_suffix}")) {
            return Some(String::from_utf8_lossy(&a.value).to_string());
        }
    }
    None
}

fn parse_coords(text: &str, dim: usize) -> Vec<[f64; 2]> {
    let vals: Vec<f64> = text
        .split_whitespace()
        .filter_map(|v| v.parse::<f64>().ok())
        .collect();
    let d = dim.max(2);
    vals.chunks(d)
        .filter(|c| c.len() >= 2)
        .map(|c| [round2(c[0]), round2(c[1])])
        .collect()
}

fn round2(v: f64) -> f64 {
    bg_round2(v)
}

/// KL-code (bronhoudercode) uit een gml:id als "nl.imkl-KL1220._x" of "nl.imkl-KL1001...".
fn kl_code(gml_id: &str) -> Option<String> {
    let bytes = gml_id.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if &gml_id[i..i + 2] == "KL" {
            let rest: String = gml_id[i + 2..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if rest.len() >= 4 {
                return Some(format!("KL{rest}"));
            }
        }
    }
    None
}

/// Parseert de GI-XML van één levering.
pub fn parse_gi_xml(xml: &str, meldnummer: &str) -> Result<Levering, String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut cur: Option<Element> = None;
    let mut geo_depth: usize = 0;
    let mut label_depth: usize = 0;
    let mut in_organisatie = false;
    let mut capture: Option<&'static str> = None;
    let mut cur_dim: usize = 2;
    let mut diameter_uom = String::from("mm");

    let mut utility_links: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut beheerders: HashMap<String, String> = HashMap::new();
    let mut netten: HashMap<String, String> = HashMap::new(); // Utiliteitsnet gml:id → thema
    let mut pending: Vec<Element> = Vec::new();
    let mut graafpolygonen: Vec<Vec<[f64; 2]>> = Vec::new();

    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("XML-fout op positie {}: {e}", reader.buffer_position()))?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = local_name(e.name().as_ref());
                let is_empty = matches!(ev, Event::Empty(_));

                if cur.is_none() {
                    let start_kind = if LIJN_ELEMENTEN.contains(&name.as_str())
                        || name == "Appurtenance"
                        || name == "Graafpolygoon"
                        || name == "UtilityLink"
                        || name == "Beheerder"
                        || name == "Utiliteitsnet"
                    {
                        Some(name.clone())
                    } else {
                        None
                    };
                    if let Some(kind) = start_kind {
                        let mut el = Element::default();
                        el.kind = kind;
                        el.gml_id = attr_value(e, "id").unwrap_or_default();
                        cur = Some(el);
                        geo_depth = 0;
                        label_depth = 0;
                        in_organisatie = false;
                        if is_empty {
                            cur = None; // leeg element, niets te doen
                        }
                        buf.clear();
                        continue;
                    }
                }

                if let Some(el) = cur.as_mut() {
                    match name.as_str() {
                        "thema" => {
                            if let Some(h) = attr_value(e, "href") {
                                el.thema = Some(href_segment(&h));
                            }
                        }
                        "link" => {
                            if let Some(h) = attr_value(e, "href") {
                                el.links.push(h.trim_start_matches('#').to_string());
                            }
                        }
                        "inNetwork" => {
                            if let Some(h) = attr_value(e, "href") {
                                el.in_network.push(h.trim_start_matches('#').to_string());
                            }
                        }
                        "appurtenanceType" => {
                            if let Some(h) = attr_value(e, "href") {
                                el.app_type = Some(href_segment(&h));
                            }
                        }
                        "buismateriaalType" | "pipeMaterialType" => {
                            if let Some(h) = attr_value(e, "href") {
                                el.materiaal = Some(href_segment(&h));
                            }
                        }
                        "pipeDiameter" => {
                            // uom kan een URN zijn (urn:ogc:def:uom:OGC::mm) → laatste segment
                            diameter_uom = attr_value(e, "uom")
                                .map(|u| {
                                    u.rsplit(|c| c == ':' || c == '/')
                                        .next()
                                        .unwrap_or("mm")
                                        .to_string()
                                })
                                .unwrap_or_else(|| "mm".to_string());
                            if !is_empty {
                                capture = Some("diameter");
                            }
                        }
                        "verticalPosition" if !is_empty => capture = Some("vertical"),
                        "bronhoudercode" if !is_empty => capture = Some("bronhoudercode"),
                        "Organisatie" => in_organisatie = true,
                        "naam" if in_organisatie && !is_empty => capture = Some("organisatie"),
                        "labelpositie" if !is_empty => label_depth += 1,
                        n if GEO_CONTAINERS.contains(&n) && !is_empty => geo_depth += 1,
                        "posList" | "pos" => {
                            cur_dim = attr_value(e, "srsDimension")
                                .and_then(|d| d.parse().ok())
                                .unwrap_or(2);
                            if geo_depth > 0 && label_depth == 0 && !is_empty {
                                capture = Some(if name == "pos" { "pos" } else { "poslist" });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Text(t) => {
                if let (Some(el), Some(cap)) = (cur.as_mut(), capture) {
                    let txt = t.unescape().unwrap_or_default().trim().to_string();
                    if !txt.is_empty() {
                        match cap {
                            "poslist" => {
                                let coords = parse_coords(&txt, cur_dim);
                                if coords.len() >= 2 {
                                    if el.kind == "Graafpolygoon" {
                                        el.ring.get_or_insert(coords);
                                    } else {
                                        el.lines.push(coords);
                                    }
                                }
                            }
                            "pos" => {
                                let c = parse_coords(&txt, cur_dim);
                                if let Some(p) = c.first() {
                                    el.point = Some(*p);
                                }
                            }
                            "diameter" => {
                                el.diameter = Some(format!("{txt} {diameter_uom}"));
                            }
                            "vertical" => el.vertical = Some(txt),
                            "bronhoudercode" => el.bronhoudercode = Some(txt),
                            "organisatie" => {
                                if el.organisatie.is_none() {
                                    el.organisatie = Some(txt);
                                }
                            }
                            _ => {}
                        }
                    }
                    capture = None;
                }
            }
            Event::End(ref e) => {
                let name = local_name(e.name().as_ref());
                capture = None;
                if let Some(el) = cur.as_mut() {
                    if name == "labelpositie" && label_depth > 0 {
                        label_depth -= 1;
                    } else if GEO_CONTAINERS.contains(&name.as_str()) && geo_depth > 0 {
                        geo_depth -= 1;
                    } else if name == "Organisatie" {
                        in_organisatie = false;
                    } else if name == el.kind {
                        let done = cur.take().unwrap();
                        match done.kind.as_str() {
                            "UtilityLink" => {
                                if let Some(line) = done.lines.into_iter().next() {
                                    utility_links.insert(done.gml_id, line);
                                }
                            }
                            "Beheerder" => {
                                if let (Some(code), Some(naam)) =
                                    (done.bronhoudercode.clone(), done.organisatie.clone())
                                {
                                    beheerders.insert(code, naam);
                                }
                            }
                            "Utiliteitsnet" => {
                                if let Some(t) = done.thema {
                                    netten.insert(done.gml_id, t);
                                }
                            }
                            "Graafpolygoon" => {
                                if let Some(ring) = done.ring {
                                    graafpolygonen.push(ring);
                                }
                            }
                            _ => pending.push(done),
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // Features samenstellen
    let mut features: Vec<Value> = Vec::new();

    for ring in &graafpolygonen {
        let mut r = ring.clone();
        if r.first() != r.last() {
            if let Some(f) = r.first().copied() {
                r.push(f);
            }
        }
        features.push(json!({
            "type": "Feature",
            "properties": { "thema": "graafpolygoon", "type": "Graafpolygoon", "melding": meldnummer },
            "geometry": { "type": "Polygon", "coordinates": [r] }
        }));
    }

    for el in pending {
        // geometrie: eigen lijnen, anders via UtilityLink-verwijzingen
        let mut lines = el.lines.clone();
        if lines.is_empty() {
            for l in &el.links {
                if let Some(line) = utility_links.get(l) {
                    lines.push(line.clone());
                }
            }
        }
        let geometry = if let Some(p) = el.point {
            json!({ "type": "Point", "coordinates": p })
        } else if lines.len() == 1 {
            json!({ "type": "LineString", "coordinates": lines[0] })
        } else if lines.len() > 1 {
            json!({ "type": "MultiLineString", "coordinates": lines })
        } else {
            continue; // annotatie-loos element zonder geometrie
        };

        let code = kl_code(&el.gml_id);
        let netbeheerder = code
            .as_ref()
            .and_then(|c| beheerders.get(c).cloned())
            .or_else(|| code.clone());

        // thema: direct op het element, anders via het Utiliteitsnet (inNetwork)
        let thema = el
            .thema
            .clone()
            .or_else(|| {
                el.in_network
                    .iter()
                    .find_map(|n| netten.get(n).cloned())
            })
            .unwrap_or_else(|| "overig".into());

        // Baken-profiel: genormaliseerde kern + bron-object met originele attributen
        let mut kern = Map::new();
        kern.insert("thema".into(), Value::String(thema));
        kern.insert("type".into(), Value::String(el.kind.clone()));
        if let Some(n) = netbeheerder {
            kern.insert("netbeheerder".into(), Value::String(n));
        }
        if let Some(d) = el.diameter.clone() {
            kern.insert("diameter".into(), Value::String(d));
        }
        if let Some(m) = el.materiaal.clone() {
            kern.insert("materiaal".into(), Value::String(m));
        }
        kern.insert("melding".into(), Value::String(meldnummer.to_string()));

        let mut bron = Map::new();
        if !el.gml_id.is_empty() {
            bron.insert("gmlId".into(), Value::String(el.gml_id.clone()));
        }
        if let Some(c) = code {
            bron.insert("bronhoudercode".into(), Value::String(c));
        }
        if let Some(v) = el.vertical {
            bron.insert("verticalPosition".into(), Value::String(v));
        }
        if let Some(t) = el.app_type {
            bron.insert("appurtenanceType".into(), Value::String(t));
        }

        features.push(baken_geo::feature(kern, bron, geometry));
    }

    Ok(Levering {
        meldnummer: meldnummer.to_string(),
        features,
    })
}

/// Leest één KLIC-zip van schijf en geeft de levering terug.
pub fn convert_zip(path: &Path) -> Result<Levering, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    convert_zip_bytes(&bytes, &path.display().to_string())
}

/// Leest één KLIC-zip uit geheugen (ook bruikbaar in WASM — geen filesystem).
pub fn convert_zip_bytes(bytes: &[u8], bron_naam: &str) -> Result<Levering, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("{bron_naam}: zip: {e}"))?;

    let mut gi_name: Option<String> = None;
    for i in 0..archive.len() {
        let name = archive.by_index(i).map_err(|e| e.to_string())?.name().to_string();
        let base = name.rsplit('/').next().unwrap_or(&name);
        if base.starts_with("GI_gebiedsinformatielevering") && base.ends_with(".xml") {
            gi_name = Some(name);
            break;
        }
    }
    let gi_name = gi_name.ok_or_else(|| {
        format!("{bron_naam}: geen GI_gebiedsinformatielevering-XML gevonden (geen KLIC-levering?)")
    })?;

    // meldnummer uit de bestandsnaam: GI_gebiedsinformatielevering_<nr>_<volg>_V2.xml
    let meldnummer = gi_name
        .rsplit('/')
        .next()
        .unwrap_or(&gi_name)
        .trim_start_matches("GI_gebiedsinformatielevering_")
        .split('_')
        .next()
        .unwrap_or("onbekend")
        .to_string();

    let mut xml = String::new();
    archive
        .by_name(&gi_name)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;

    parse_gi_xml(&xml, &meldnummer)
}

/// Combineert leveringen tot één Baken-profiel FeatureCollection (envelop
/// met schema, laagtype, bron en samenvatting — zie baken-geo).
pub fn feature_collection(
    name: &str,
    project: &str,
    bestanden: &[String],
    leveringen: &[Levering],
) -> Value {
    let mut features: Vec<Value> = Vec::new();
    let mut meldnummers: Vec<String> = Vec::new();
    for lev in leveringen {
        meldnummers.push(lev.meldnummer.clone());
        features.extend(lev.features.iter().cloned());
    }
    let bron = Bron {
        formaat: "klic-zip".into(),
        bestanden: bestanden.to_vec(),
        meldnummers,
    };
    baken_geo::envelope(name, Laagtype::Klic, project, &bron, features)
}
