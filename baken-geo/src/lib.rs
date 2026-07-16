//! Baken GeoJSON-profiel v1 — het gedeelde datacontract van alle importers.
//!
//! Elke converter (klic2geo, nlcs2geo, plan2geo, …) levert een
//! FeatureCollection met een `baken`-envelop:
//!
//! ```json
//! { "type": "FeatureCollection", "name": "…",
//!   "baken": { "schema": "baken-geo/1", "laagtype": "klic", "project": "…",
//!              "bron": {…}, "samenvatting": {…} },
//!   "features": [ … ] }
//! ```
//!
//! Feature-properties: genormaliseerde kern (`thema`, `type`, `netbeheerder`,
//! `melding`, …) plus een `bron`-object met alle originele attributen.
//! Coördinaten: RD (EPSG:28992), 2 decimalen.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "baken-geo/1";

/// Laagtype in het profiel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Laagtype {
    Klic,
    Ontwerp,
    Onderlegger,
    Meetpunten,
    Projecten,
}

impl Laagtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Laagtype::Klic => "klic",
            Laagtype::Ontwerp => "ontwerp",
            Laagtype::Onderlegger => "onderlegger",
            Laagtype::Meetpunten => "meetpunten",
            Laagtype::Projecten => "projecten",
        }
    }
}

/// Broninformatie van een import.
#[derive(Default, Clone, Debug)]
pub struct Bron {
    pub formaat: String,           // "klic-zip" | "nlcs-xml" | "dxf" | "dwg" | "pdf"
    pub bestanden: Vec<String>,
    pub meldnummers: Vec<String>,  // KLIC-meldnummers indien van toepassing
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Bouwt één feature met kern-properties + `bron`-object.
pub fn feature(kern: Map<String, Value>, bron: Map<String, Value>, geometry: Value) -> Value {
    let mut props = kern;
    if !bron.is_empty() {
        props.insert("bron".into(), Value::Object(bron));
    }
    json!({ "type": "Feature", "properties": Value::Object(props), "geometry": geometry })
}

/// Bouwt de profiel-envelop rond een verzameling features en berekent de
/// samenvatting (aantallen, themas, netbeheerders, bbox, center).
pub fn envelope(
    name: &str,
    laagtype: Laagtype,
    project: &str,
    bron: &Bron,
    features: Vec<Value>,
) -> Value {
    let mut themas: BTreeMap<String, usize> = BTreeMap::new();
    let mut netbeheerders: BTreeSet<String> = BTreeSet::new();
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);

    for f in &features {
        if let Some(t) = f["properties"]["thema"].as_str() {
            *themas.entry(t.to_string()).or_insert(0) += 1;
        }
        if let Some(n) = f["properties"]["netbeheerder"].as_str() {
            netbeheerders.insert(n.to_string());
        }
        walk_coords(&f["geometry"]["coordinates"], &mut |x, y| {
            xmin = xmin.min(x);
            ymin = ymin.min(y);
            xmax = xmax.max(x);
            ymax = ymax.max(y);
        });
    }

    let bbox_ok = xmin != f64::MAX;
    json!({
        "type": "FeatureCollection",
        "name": name,
        "baken": {
            "schema": SCHEMA,
            "laagtype": laagtype.as_str(),
            "project": project,
            "bron": {
                "formaat": bron.formaat,
                "bestanden": bron.bestanden,
                "meldnummers": bron.meldnummers,
            },
            "samenvatting": {
                "features": features.len(),
                "themas": themas,
                "netbeheerders": netbeheerders.into_iter().collect::<Vec<_>>(),
                "bbox": if bbox_ok { json!([round2(xmin), round2(ymin), round2(xmax), round2(ymax)]) } else { Value::Null },
                "center": if bbox_ok { json!([round2((xmin+xmax)/2.0), round2((ymin+ymax)/2.0)]) } else { Value::Null },
            }
        },
        "features": features
    })
}

/// Loopt over alle coördinaatparen in een GeoJSON-coordinates-waarde.
pub fn walk_coords(v: &Value, f: &mut impl FnMut(f64, f64)) {
    if let Some(arr) = v.as_array() {
        if arr.len() >= 2 && arr[0].is_number() && arr[1].is_number() {
            f(arr[0].as_f64().unwrap_or(0.0), arr[1].as_f64().unwrap_or(0.0));
        } else {
            for item in arr {
                walk_coords(item, f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_berekent_samenvatting() {
        let mut kern = Map::new();
        kern.insert("thema".into(), json!("water"));
        kern.insert("netbeheerder".into(), json!("Brabant Water"));
        let f = feature(
            kern,
            Map::new(),
            json!({"type":"LineString","coordinates":[[100000.0,400000.0],[100010.0,400010.0]]}),
        );
        let fc = envelope(
            "test",
            Laagtype::Klic,
            "proj-x",
            &Bron { formaat: "klic-zip".into(), bestanden: vec!["a.zip".into()], meldnummers: vec!["26G1".into()] },
            vec![f],
        );
        assert_eq!(fc["baken"]["schema"], SCHEMA);
        assert_eq!(fc["baken"]["samenvatting"]["features"], 1);
        assert_eq!(fc["baken"]["samenvatting"]["themas"]["water"], 1);
        assert_eq!(fc["baken"]["samenvatting"]["center"], json!([100005.0, 400005.0]));
    }
}
