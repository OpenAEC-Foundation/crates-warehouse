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
/// Emits the standard headers (`#GEFID`, `#FILEOWNER`, `#FILEDATE`, `#PROJECTID`,
/// `#TESTID`, `#COMPANYID`, `#XYID`, `#ZID`, `#COLUMN= 4`, four `#COLUMNINFO=`
/// lines for length / qc / fs / rf, `#COLUMNVOID=`, `#COLUMNSEPARATOR= ;`,
/// `#RECORDSEPARATOR= !`, `#EOH=`) followed by `;`-separated rows terminated
/// by `!`.
///
/// `#FILEOWNER` and `#PROJECTID` are mandatory per GEF 1.0/1.1 spec; the
/// validator (GEFPlotTool 5.1) rejects files without them. Missing values
/// fall back to a sensible default (Open Geotechniek Studio / Unknown) so
/// the output always validates.
///
/// The data section uses exactly the declared separators with no extra
/// whitespace: `value;value;value;value!` per record. Earlier versions
/// padded with spaces (`value ; value ; ...`), which the strict validator
/// rejected with "No valid columnseparator was found".
pub fn write_gef(cpt: &Cpt) -> String {
    let m = &cpt.metadata;
    let mut out = String::new();

    out.push_str("#GEFID= 1, 1, 0\n");
    // #FILEOWNER is a mandatory header keyword in GEF 1.x; the strict
    // validator (GEFPlotTool 5.1) rejects files without it. We always
    // emit it — defaulting to the Studio brand if no equipment owner
    // was captured by the parser.
    let file_owner = m
        .extra
        .get("FILEOWNER")
        .cloned()
        .unwrap_or_else(|| "Open Geotechniek Studio".to_string());
    out.push_str(&format!("#FILEOWNER= {}\n", file_owner));
    if let Some(d) = m.date {
        out.push_str(&format!(
            "#FILEDATE= {}, {}, {}\n",
            d.format("%Y"),
            d.format("%m"),
            d.format("%d")
        ));
    }
    // #PROJECTID is also mandatory. Fall back through the typed field,
    // the parser's `extra` bag, and finally a deterministic placeholder
    // so the file still validates round-trip.
    let project_id = m
        .project_number
        .clone()
        .or_else(|| m.extra.get("PROJECTID").cloned())
        .unwrap_or_else(|| "Unknown".to_string());
    out.push_str(&format!("#PROJECTID= {}\n", project_id));
    out.push_str(&format!("#TESTID= {}\n", cpt.id));
    if let Some(name) = m.project_name.as_deref() {
        out.push_str(&format!("#PROJECTNAME= {}\n", name));
    }
    if let Some(comp) = m.equipment.as_deref() {
        out.push_str(&format!("#COMPANYID= {}\n", comp));
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
    // #REPORTCODE markeert dit bestand als een CPT-rapport voor
    // de officiële validator (GEFPlotTool 5.1). Zonder deze regel
    // faalt de validator met "GEF-CPT-Report 110: This is not a
    // CPT Report". Format: naam, major, minor, patch (zelfde als
    // BRO's IMBRO-A exports).
    out.push_str("#REPORTCODE= GEF-CPT-Report, 1, 1, 2\n");
    out.push_str("#EOH=\n");

    // Data rows: alle waarden ge-scheiden door de gedeclareerde `;`
    // INCLUSIEF na de laatste waarde, daarna de record-terminator
    // `!` en een newline. Dus: `0.000;1.500;0.020;1.000;!\n`
    //
    // GEFPlotTool 5.1 verwacht dat ELKE waarde gevolgd wordt door
    // de COLUMNSEPARATOR — ook de laatste, vóór de RECORDSEPARATOR.
    // Zonder die laatste `;` faalt het met "No valid columnseparator
    // was found after scan 1, column N". Onze BRO sample-GEFs doen
    // hetzelfde (`...;6.3;!\n`).
    for p in &cpt.points {
        out.push_str(&format!(
            "{:.3};{};{};{};!\n",
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

    /// GEFPlotTool 5.1 demands `#FILEOWNER=` and `#PROJECTID=` as
    /// mandatory header keywords, and rejects data rows that pad values
    /// with whitespace around the declared `#COLUMNSEPARATOR`. This test
    /// pins those three correctness requirements.
    #[test]
    fn gef_writes_required_headers() {
        let cpt = sample_cpt();
        let gef = write_gef(&cpt);

        // Mandatory header keywords (per GEF 1.x / GEFPlotTool 5.1).
        assert!(
            gef.contains("#FILEOWNER="),
            "GEF output must contain #FILEOWNER= (mandatory header)\n--- GEF ---\n{}",
            gef
        );
        assert!(
            gef.contains("#PROJECTID="),
            "GEF output must contain #PROJECTID= (mandatory header)\n--- GEF ---\n{}",
            gef
        );

        // The data section must match the declared #COLUMNSEPARATOR= ;
        // EXACTLY — no spaces around the `;` separator and no space
        // before the `!` record terminator. Earlier output looked like
        // `0.020 ; 1.500 ; 0.015 ; 1.000 !\n`, which the strict
        // validator rejects with "No valid columnseparator was found".
        let data_start = gef.find("#EOH=").expect("missing EOH") + "#EOH=\n".len();
        let data_section = &gef[data_start..];
        assert!(
            data_section.contains(";"),
            "data section must contain `;` separator: {:?}",
            data_section
        );
        assert!(
            !data_section.contains(" ;") && !data_section.contains("; "),
            "data section must not pad `;` with spaces:\n{}",
            data_section
        );
        assert!(
            !data_section.contains(" !"),
            "data section must not have space before `!` record terminator:\n{}",
            data_section
        );

        // And the writer's output must still round-trip through our own parser.
        let back = parse_gef(&gef).expect("write_gef output should still parse");
        assert_eq!(back.points.len(), cpt.points.len());
    }

    /// When the metadata has no project_number and no PROJECTID extra,
    /// the writer must still emit a `#PROJECTID=` line so the validator
    /// doesn't reject the file. (Default value is `Unknown`.)
    #[test]
    fn gef_emits_projectid_even_when_missing() {
        let mut cpt = sample_cpt();
        cpt.metadata.project_number = None;
        cpt.metadata.extra.clear();
        let gef = write_gef(&cpt);
        assert!(
            gef.contains("#PROJECTID="),
            "PROJECTID must be emitted with a fallback when source has none"
        );
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
