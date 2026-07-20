//! Baken GeoJSON-profiel (laagtype `ontwerp`) → CAD (DXF/DWG) via RUST-DWG.
//!
//! De omgekeerde weg van de import: de GIS-ontwerplaag wordt teruggeschreven
//! naar een CAD-tekening. Elk feature komt op een laag `<objecttype> <status>`
//! (NLCS-achtig), lijnen als LwPolyline, punten als Point. De bytes komen uit
//! acadrust's `DxfWriter`/`DwgWriter` — hetzelfde RUST-DWG dat Open CAD Studio
//! aandrijft.

use acadrust::entities::lwpolyline::LwVertex;
use acadrust::tables::Layer;
use acadrust::types::Color;
use acadrust::{CadDocument, DwgWriter, DxfWriter, EntityType, LwPolyline, Point};
use acadrust::{Vector2, Vector3};
use serde_json::Value;
use std::collections::BTreeMap;

fn v3(x: f64, y: f64) -> Vector3 {
    Vector3 { x, y, z: 0.0 }
}

/// NLCS-laagnaam per feature: `<objecttype-of-thema>_<status>`
/// (bv. `MSkabel_NIEUW`). Het objecttype ís de NLCS++-elementnaam.
fn laag_van(props: &Value) -> String {
    let ot = props["objecttype"]
        .as_str()
        .or_else(|| props["thema"].as_str())
        .unwrap_or("overig");
    let st = props["status"].as_str().unwrap_or("BESTAAND");
    format!("{ot}_{st}")
}

/// AutoCAD-kleurindex (ACI) per PMKL-thema — dezelfde kleurtaal als de
/// KLIC-/ontwerplaag in het GIS, zodat de DWG herkenbaar opent.
fn aci_van_thema(thema: &str) -> i16 {
    match thema {
        "middenspanning" | "laagspanning" | "hoogspanning" | "landelijkHoogspanningsnet" => 1, // rood
        "gasLageDruk" | "gasHogeDruk" => 2,       // geel
        "water" => 5,                              // blauw
        "datatransport" => 3,                      // groen
        "warmte" => 6,                             // magenta
        "rioolVrijverval" | "rioolOnderOverOfOnderdruk" => 32, // bruin
        "topografie" | "maatvoering" => 8,         // grijs
        _ => 7,                                     // wit/zwart
    }
}

fn coord(c: &Value) -> Option<(f64, f64)> {
    let a = c.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

/// Voegt één lijnstring als LwPolyline toe. Geeft 1 bij succes, 0 bij te weinig punten.
fn add_lijn(doc: &mut CadDocument, coords: &Value, laag: &str) -> Result<usize, String> {
    let pts: Vec<(f64, f64)> = coords
        .as_array()
        .map(|a| a.iter().filter_map(coord).collect())
        .unwrap_or_default();
    if pts.len() < 2 {
        return Ok(0);
    }
    let mut pl = LwPolyline::new();
    for (x, y) in pts {
        pl.add_vertex(LwVertex::new(Vector2 { x, y }));
    }
    pl.common.layer = laag.to_string();
    doc.add_entity(EntityType::LwPolyline(pl))
        .map_err(|e| e.to_string())?;
    Ok(1)
}

/// Bouwt een CadDocument uit de Baken-ontwerplaag. Geeft (document, aantal entiteiten).
pub fn geojson_to_document(geojson: &str) -> Result<(CadDocument, usize), String> {
    let fc: Value = serde_json::from_str(geojson).map_err(|e| format!("GeoJSON: {e}"))?;
    let mut doc = CadDocument::new();
    let mut n = 0usize;
    let leeg = Vec::new();
    let features = fc["features"].as_array().unwrap_or(&leeg);

    // 1. NLCS-lagen vooraf registreren, ingekleurd per PMKL-thema. Zo opent de
    //    DWG met de juiste laagstructuur en kleuren i.p.v. alles op laag 0.
    let mut lagen: BTreeMap<String, i16> = BTreeMap::new();
    for f in features {
        let props = &f["properties"];
        let thema = props["thema"].as_str().unwrap_or("overig");
        lagen.entry(laag_van(props)).or_insert_with(|| aci_van_thema(thema));
    }
    for (naam, aci) in &lagen {
        let mut laag = Layer::new(naam.clone());
        laag.color = Color::from_index(*aci);
        doc.layers.add(laag).ok();
    }

    for f in features {
        let props = &f["properties"];
        let laag = laag_van(props);
        let g = &f["geometry"];
        match g["type"].as_str().unwrap_or("") {
            "Point" => {
                if let Some((x, y)) = coord(&g["coordinates"]) {
                    let mut p = Point::new();
                    p.location = v3(x, y);
                    p.common.layer = laag.clone();
                    doc.add_entity(EntityType::Point(p)).map_err(|e| e.to_string())?;
                    n += 1;
                }
            }
            "LineString" => n += add_lijn(&mut doc, &g["coordinates"], &laag)?,
            "MultiLineString" => {
                if let Some(lijnen) = g["coordinates"].as_array() {
                    for lijn in lijnen {
                        n += add_lijn(&mut doc, lijn, &laag)?;
                    }
                }
            }
            "Polygon" => {
                if let Some(ringen) = g["coordinates"].as_array() {
                    for ring in ringen {
                        n += add_lijn(&mut doc, ring, &laag)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok((doc, n))
}

/// GeoJSON-ontwerplaag → DXF-bytes (ASCII interchange, opent in elk CAD-pakket).
pub fn geojson_to_dxf(geojson: &str) -> Result<Vec<u8>, String> {
    let (doc, _) = geojson_to_document(geojson)?;
    DxfWriter::new(&doc)
        .write_to_vec()
        .map_err(|e| format!("DXF-schrijven: {e}"))
}

/// GeoJSON-ontwerplaag → DWG-bytes (native, via RUST-DWG).
pub fn geojson_to_dwg(geojson: &str) -> Result<Vec<u8>, String> {
    let (doc, _) = geojson_to_document(geojson)?;
    DwgWriter::write_to_vec(&doc).map_err(|e| format!("DWG-schrijven: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"objecttype":"MSkabel","status":"NIEUW","thema":"middenspanning"},
       "geometry":{"type":"LineString","coordinates":[[166200,419300],[166250,419320],[166300,419300]]}},
      {"type":"Feature","properties":{"objecttype":"MSmof","status":"NIEUW","thema":"middenspanning"},
       "geometry":{"type":"Point","coordinates":[166250,419320]}}
    ]}"#;

    #[test]
    fn document_bevat_entiteiten() {
        let (doc, n) = geojson_to_document(FC).unwrap();
        assert_eq!(n, 2);
        assert_eq!(doc.entities().count(), 2);
    }

    #[test]
    fn dxf_rondrit() {
        let bytes = geojson_to_dxf(FC).unwrap();
        assert!(bytes.len() > 100, "dxf te klein: {}", bytes.len());
        // terug inlezen: moet dezelfde 2 entiteiten opleveren
        let doc = acadrust::DxfReader::from_reader(std::io::Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        let lagen: Vec<String> = doc.entities().map(|e| e.common().layer.clone()).collect();
        assert_eq!(lagen.len(), 2, "verwacht 2 entiteiten, kreeg {}", lagen.len());
        assert!(lagen.iter().any(|l| l == "MSkabel_NIEUW"), "lagen: {lagen:?}");
        assert!(lagen.iter().any(|l| l == "MSmof_NIEUW"), "lagen: {lagen:?}");
    }

    #[test]
    fn nlcs_lagen_met_kleur() {
        let (doc, _) = geojson_to_document(FC).unwrap();
        // MSkabel_NIEUW + MSmof_NIEUW als aparte lagen, ingekleurd rood (ACI 1)
        let namen: Vec<String> = doc.layers.iter().map(|l| l.name.clone()).collect();
        assert!(namen.iter().any(|n| n == "MSkabel_NIEUW"), "lagen: {namen:?}");
        assert!(namen.iter().any(|n| n == "MSmof_NIEUW"), "lagen: {namen:?}");
        let kabel = doc.layers.iter().find(|l| l.name == "MSkabel_NIEUW").unwrap();
        assert_eq!(kabel.color, Color::from_index(1), "middenspanning = rood");
    }

    #[test]
    fn dwg_schrijft_bytes() {
        let bytes = geojson_to_dwg(FC).unwrap();
        assert!(bytes.len() > 100, "dwg te klein: {}", bytes.len());
        assert_eq!(&bytes[0..2], b"AC", "geen DWG-magic");
    }
}
