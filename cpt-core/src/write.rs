//! Serializers — write an in-memory `Cpt` back into the source formats
//! (GEF, BRO XML) or the per-CPT IfcGeo JSON snapshot.
//!
//! The output is intentionally minimal but round-trips through the
//! corresponding parser in this crate. A perfectly schema-valid BRO XML
//! deliverable would require quite a bit more (BRO_id assignment, namespaces,
//! quality regime, …); this writer is meant for end-user *export* of an
//! in-memory CPT (e.g. converting a GEF into a BRO-style XML for sharing),
//! not for re-uploading to BRO.

use chrono::NaiveDate;

use crate::domain::Cpt;
use crate::error::CptError;

const GEF_VOID: f64 = -9999.0;

/// Serialize a CPT as a GEF document (Dutch CPT exchange format).
///
/// Emits the standard headers (`#GEFID`, `#TESTID`, `#PROJECTID`, `#COMPANYID`,
/// `#FILEDATE`, `#XYID`, `#ZID`, `#COLUMN= 4`, four `#COLUMNINFO=` lines
/// for length / qc / fs / rf, `#COLUMNVOID= 2, -9999`, `#EOH=`) followed by
/// space-separated rows. Missing fields are emitted as `-9999`.
pub fn write_gef(cpt: &Cpt) -> String {
    let m = &cpt.metadata;
    let mut out = String::new();

    out.push_str("#GEFID= 1, 1, 0\n");
    out.push_str(&format!("#TESTID= {}\n", cpt.id));
    if let Some(pid) = m.project_number.as_deref() {
        out.push_str(&format!("#PROJECTID= {}\n", pid));
    } else if let Some(pid) = m.extra.get("PROJECTID") {
        out.push_str(&format!("#PROJECTID= {}\n", pid));
    }
    if let Some(name) = m.project_name.as_deref() {
        out.push_str(&format!("#PROJECTNAME= {}\n", name));
    }
    if let Some(comp) = m.equipment.as_deref() {
        out.push_str(&format!("#COMPANYID= {}\n", comp));
    }
    if let Some(d) = m.date {
        out.push_str(&format!(
            "#FILEDATE= {}, {}, {}\n",
            d.format("%Y"),
            d.format("%m"),
            d.format("%d")
        ));
    }
    if let Some(pos) = cpt.position {
        out.push_str(&format!("#XYID= 31000, {:.3}, {:.3}\n", pos.x_rd, pos.y_rd));
        if let Some(z) = pos.z_nap {
            out.push_str(&format!("#ZID= 31000, {:.3}\n", z));
        }
    } else if let Some(z) = m.ground_level_nap {
        out.push_str(&format!("#ZID= 31000, {:.3}\n", z));
    }

    // Fixed 4-column layout: length, qc, fs, rf.
    out.push_str("#COLUMN= 4\n");
    out.push_str("#COLUMNINFO= 1, m, Sondeerlengte, 1\n");
    out.push_str("#COLUMNINFO= 2, MPa, Conusweerstand, 2\n");
    out.push_str("#COLUMNINFO= 3, MPa, Plaatselijke wrijving, 3\n");
    out.push_str("#COLUMNINFO= 4, %, Wrijvingsgetal, 4\n");
    out.push_str(&format!("#COLUMNVOID= 2, {}\n", GEF_VOID));
    out.push_str(&format!("#COLUMNVOID= 3, {}\n", GEF_VOID));
    out.push_str(&format!("#COLUMNVOID= 4, {}\n", GEF_VOID));
    out.push_str("#COLUMNSEPARATOR= ;\n");
    out.push_str("#RECORDSEPARATOR= !\n");
    out.push_str("#EOH=\n");

    for p in &cpt.points {
        out.push_str(&format!(
            "{:.3} ; {} ; {} ; {} !\n",
            p.depth,
            fmt_opt(p.qc),
            fmt_opt(p.fs),
            fmt_opt(p.rf),
        ));
    }
    out
}

fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.4}", x),
        None => format!("{}", GEF_VOID),
    }
}

/// Serialize a CPT as a minimal BRO XML document (CPT_O dispatch envelope).
///
/// Emits enough structure that the BRO parser in this crate round-trips
/// the data: broId, deliveredLocation/pos (RD), deliveredVerticalPosition/offset,
/// researchReportDate, and a 25-column `<cptcommon:values>` data block in the
/// canonical order expected by `bro::columns::ORDER`. Missing fields are
/// emitted as the BRO void value (-999999).
pub fn write_bro_xml(cpt: &Cpt) -> String {
    let m = &cpt.metadata;
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
"#);
    out.push_str(r#"<dispatchDataResponse xmlns="http://www.broservices.nl/xsd/dscpt/1.1" xmlns:swe="http://www.opengis.net/swe/2.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:brocom="http://www.broservices.nl/xsd/brocommon/3.0" xmlns:cptcommon="http://www.broservices.nl/xsd/cptcommon/1.1" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:om="http://www.opengis.net/om/2.0" xmlns:sampling="http://www.opengis.net/sampling/2.0">
"#);
    out.push_str("  <brocom:responseType>dispatch</brocom:responseType>\n");
    out.push_str("  <dispatchDocument>\n");
    out.push_str(r#"    <CPT_O gml:id="OGS_0001">
"#);
    out.push_str(&format!(
        "      <brocom:broId>{}</brocom:broId>\n",
        xml_escape(&cpt.id)
    ));
    if let Some(date) = m.date {
        out.push_str("      <researchReportDate>\n");
        out.push_str(&format!(
            "        <brocom:date>{}</brocom:date>\n",
            date.format("%Y-%m-%d")
        ));
        out.push_str("      </researchReportDate>\n");
    }
    if let Some(pos) = cpt.position {
        out.push_str(r#"      <deliveredLocation>
"#);
        out.push_str(r#"        <cptcommon:location srsName="urn:ogc:def:crs:EPSG::28992" gml:id="OGS_LOC_0001">
"#);
        out.push_str(&format!(
            "          <gml:pos>{:.3} {:.3}</gml:pos>\n",
            pos.x_rd, pos.y_rd
        ));
        out.push_str("        </cptcommon:location>\n");
        out.push_str("      </deliveredLocation>\n");
        if let Some(z) = pos.z_nap.or(m.ground_level_nap) {
            out.push_str("      <deliveredVerticalPosition>\n");
            out.push_str(&format!(
                "        <cptcommon:offset uom=\"m\">{:.3}</cptcommon:offset>\n",
                z
            ));
            out.push_str("        <cptcommon:verticalDatum codeSpace=\"urn:bro:cpt:VerticalDatum\">NAP</cptcommon:verticalDatum>\n");
            out.push_str("      </deliveredVerticalPosition>\n");
        }
    } else if let Some(z) = m.ground_level_nap {
        out.push_str("      <deliveredVerticalPosition>\n");
        out.push_str(&format!(
            "        <cptcommon:offset uom=\"m\">{:.3}</cptcommon:offset>\n",
            z
        ));
        out.push_str("        <cptcommon:verticalDatum codeSpace=\"urn:bro:cpt:VerticalDatum\">NAP</cptcommon:verticalDatum>\n");
        out.push_str("      </deliveredVerticalPosition>\n");
    }
    out.push_str("      <conePenetrometerSurvey>\n");
    out.push_str("        <cptcommon:conePenetrationTest>\n");
    out.push_str("          <cptcommon:cptResult>\n");
    out.push_str("            <cptcommon:values>");
    let mut first = true;
    for p in &cpt.points {
        if !first {
            out.push(';');
        }
        first = false;
        out.push_str(&bro_row(p));
    }
    out.push_str("</cptcommon:values>\n");
    out.push_str("          </cptcommon:cptResult>\n");
    out.push_str("        </cptcommon:conePenetrationTest>\n");
    out.push_str("      </conePenetrometerSurvey>\n");
    out.push_str("    </CPT_O>\n");
    out.push_str("  </dispatchDocument>\n");
    out.push_str("</dispatchDataResponse>\n");
    out
}

/// Build one BRO row in the 25-column order used by `bro::columns::ORDER`.
/// Index→field: 0:Length, 1:Depth, 2:ElapsedTime, 3:Qc, 4:CorrectedQc,
/// 5:NetQc, 6..14:mag/incl pad, 15:Inclination, 16..17:mag pad,
/// 18:Fs, 19:PoreRatio, 20:Temp, 21:U1, 22:U2, 23:U3, 24:Rf.
fn bro_row(p: &crate::domain::MeasurementPoint) -> String {
    let v = bro_void();
    let nums: [String; 25] = [
        format!("{:.3}", p.depth),                                  // Length (use depth as length)
        format!("{:.3}", p.depth),                                  // Depth
        v.clone(),                                                  // ElapsedTime
        opt(p.qc),                                                  // Qc
        v.clone(),                                                  // CorrectedQc
        v.clone(),                                                  // NetQc
        v.clone(), v.clone(), v.clone(), v.clone(), v.clone(),     // MagX..ElectricCond
        v.clone(), v.clone(), v.clone(), v.clone(),                // InclEw..InclY
        opt(p.inclination),                                         // Inclination
        v.clone(), v.clone(),                                       // MagInclination, MagDeclination
        opt(p.fs),                                                  // Fs
        v.clone(),                                                  // PoreRatio
        v.clone(),                                                  // Temp
        v.clone(),                                                  // U1
        opt(p.u2),                                                  // U2
        v.clone(),                                                  // U3
        opt(p.rf),                                                  // Rf
    ];
    nums.join(",")
}

fn opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.3}", x),
        None => bro_void(),
    }
}

fn bro_void() -> String {
    "-999999".to_string()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Serialize a single CPT as `.ifcgeo` JSON — the Open GEO Studio per-CPT
/// snapshot format. This is *just* a CPT object (no project wrapper), in
/// the same JSON shape the cpt-core domain types use natively. Round-trips
/// through `read_ifcgeo`.
pub fn write_ifcgeo(cpt: &Cpt) -> Result<String, CptError> {
    serde_json::to_string_pretty(cpt)
        .map_err(|e| CptError::InvalidGef(format!("ifcgeo serialize: {e}")))
}

/// Parse a `.ifcgeo` JSON snapshot back into a `Cpt`.
pub fn read_ifcgeo(text: &str) -> Result<Cpt, CptError> {
    serde_json::from_str(text)
        .map_err(|e| CptError::InvalidGef(format!("ifcgeo parse: {e}")))
}

/// Helper used by the GEF writer to format a date, not currently exposed
/// publicly but kept for future test access.
#[allow(dead_code)]
fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MeasurementPoint, Metadata, Position};
    use crate::{parse_bro, parse_gef};

    fn sample_cpt() -> Cpt {
        Cpt {
            id: "S01".into(),
            metadata: Metadata {
                project_name: Some("Demo".into()),
                project_number: Some("2026-001".into()),
                date: NaiveDate::from_ymd_opt(2026, 5, 15),
                equipment: Some("Konings BV".into()),
                ground_level_nap: Some(1.25),
                source_file: "S01.gef".into(),
                extra: Default::default(),
            },
            position: Some(Position {
                x_rd: 100_000.0,
                y_rd: 400_000.0,
                z_nap: Some(1.25),
            }),
            points: vec![
                MeasurementPoint { depth: 0.02, depth_nap: Some(1.23), qc: Some(1.5), fs: Some(0.015), rf: Some(1.0), u2: None, inclination: Some(0.5) },
                MeasurementPoint { depth: 0.04, depth_nap: Some(1.21), qc: Some(1.8), fs: Some(0.018), rf: Some(1.0), u2: None, inclination: Some(0.6) },
                MeasurementPoint { depth: 0.06, depth_nap: Some(1.19), qc: Some(2.1), fs: Some(0.020), rf: None,      u2: None, inclination: Some(0.7) },
            ],
        }
    }

    #[test]
    fn gef_round_trip() {
        let original = sample_cpt();
        let gef = write_gef(&original);
        let back = parse_gef(&gef).expect("write_gef output should parse");
        assert_eq!(back.id, "S01");
        assert_eq!(back.points.len(), 3);
        assert!((back.points[0].qc.unwrap() - 1.5).abs() < 1e-3);
        assert!((back.points[1].fs.unwrap() - 0.018).abs() < 1e-4);
    }

    #[test]
    fn bro_round_trip() {
        let original = sample_cpt();
        let xml = write_bro_xml(&original);
        let back = parse_bro(&xml).expect("write_bro_xml output should parse");
        assert_eq!(back.id, "S01");
        assert_eq!(back.points.len(), 3);
        assert!((back.points[0].depth - 0.02).abs() < 1e-3);
        assert!((back.points[0].qc.unwrap() - 1.5).abs() < 1e-3);
        assert_eq!(back.position.unwrap().x_rd, 100_000.0);
    }

    #[test]
    fn ifcgeo_round_trip() {
        let original = sample_cpt();
        let json = write_ifcgeo(&original).unwrap();
        let back = read_ifcgeo(&json).unwrap();
        assert_eq!(back.id, original.id);
        assert_eq!(back.points.len(), original.points.len());
        assert_eq!(back.metadata.project_number, original.metadata.project_number);
    }
}
