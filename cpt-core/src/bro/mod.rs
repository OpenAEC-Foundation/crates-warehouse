//! BRO (Basisregistratie Ondergrond) XML parser for CPT_O / CPT_O_DP documents.
//!
//! Uses `quick-xml` for streaming parse. Extracts:
//! - broId (test id)
//! - deliveredLocation/pos (RD coordinates, EPSG:28992)
//! - deliveredVerticalPosition/offset (Z-NAP)
//! - researchReportDate
//! - 25-column SWE data array from cptResult (NOT dissipationTest)

pub mod columns;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::domain::{Cpt, MeasurementPoint, Metadata, Position};
use crate::error::CptError;
use self::columns::{BroField, ORDER, VOID_VALUE};

pub fn parse(xml: &str) -> Result<Cpt, CptError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();

    let mut id: Option<String> = None;
    let mut x: Option<f64> = None;
    let mut y: Option<f64> = None;
    let mut z: Option<f64> = None;
    let mut date: Option<NaiveDate> = None;
    let mut data_block: Option<String> = None;
    let mut extras: BTreeMap<String, String> = BTreeMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                path.push(local);
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Text(t)) => {
                let txt = t
                    .unescape()
                    .map_err(|e| CptError::InvalidBro(e.to_string()))?
                    .into_owned();
                handle_text(
                    &path,
                    &txt,
                    &mut id,
                    &mut x,
                    &mut y,
                    &mut z,
                    &mut date,
                    &mut data_block,
                    &mut extras,
                );
            }
            Ok(Event::Empty(_)) => {
                // Self-closing tag — no text, no nesting to track.
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(CptError::InvalidBro(format!(
                    "xml at pos {}: {}",
                    reader.buffer_position(),
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    let id = id.ok_or_else(|| CptError::InvalidBro("missing broId".into()))?;
    let block = data_block
        .ok_or_else(|| CptError::InvalidBro("missing cptResult values data block".into()))?;
    let mut points = parse_data_block(&block, z);

    // BRO data arrays are not guaranteed to be sorted by depth — some files
    // (e.g. CPT000000000787) interleave segments out of order. The chart
    // renders points as a single polyline, so unsorted input creates a
    // criss-crossing zigzag. Sort ascending by depth so adjacent points in
    // the polyline are also adjacent in the borehole.
    points.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));

    let position = match (x, y) {
        (Some(x), Some(y)) => Some(Position {
            x_rd: x,
            y_rd: y,
            z_nap: z,
        }),
        _ => None,
    };

    Ok(Cpt {
        id,
        metadata: Metadata {
            project_name: None,
            project_number: None,
            date,
            equipment: None,
            ground_level_nap: z,
            source_file: String::new(),
            extra: extras,
        },
        position,
        points,
    })
}

fn local_name(qname: &[u8]) -> String {
    let s = std::str::from_utf8(qname).unwrap_or("");
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_text(
    path: &[String],
    txt: &str,
    id: &mut Option<String>,
    x: &mut Option<f64>,
    y: &mut Option<f64>,
    z: &mut Option<f64>,
    date: &mut Option<NaiveDate>,
    data_block: &mut Option<String>,
    extras: &mut BTreeMap<String, String>,
) {
    let last = match path.last() {
        Some(s) => s.as_str(),
        None => return,
    };

    // Curated list of BRO single-text fields we surface as "extras".
    // Skips the big data-block fields (values, pos, offset) and anything
    // already mapped to a typed Metadata field.
    //
    // The list mirrors the field set bedrock-engineer/bro-xml-parser-ts
    // exposes as "metadata" — adds qualityClass, accuracyClass, sondage
    // identifiers, calibration descriptors, and the typed conePenetrometer
    // fields a reviewer needs but which don't fit our typed Metadata
    // schema. All are passively read; missing ones are silently skipped.
    const EXTRA_FIELDS: &[&str] = &[
        "objectIdAccountableParty",
        "qualityRegime",
        "qualityClass",
        "accuracyClass",
        "deliveryContext",
        "surveyPurpose",
        "cptStandard",
        "researchOperator",
        "researchReportSubmittedBy",
        "stopCriterion",
        "cptMethod",
        "predrilledDepth",
        "finalDepth",
        "finalDepthBoring",
        "groundwaterLevel",
        "conePenetrometerType",
        "conePenetrometerName",
        "conePenetrometerDescription",
        "coneSurfaceArea",
        "coneSurfaceQuotient",
        "frictionSleeveSurfaceArea",
        "frictionSleeveSurfaceQuotient",
        "frictionSleeveDistance",
        "conePresentationLength",
        "linearSlopeDetected",
        "verticalDatum",
        "localVerticalReferencePoint",
        "horizontalPositioningMethod",
        "deliveredVerticalPositioningMethod",
        // Calibration / dissipation context — the parser still ignores the
        // dissipationTest data array (we want cptResult/values only) but
        // surfacing the descriptive fields lets reviewers see whether one
        // was present and roughly how it was set up.
        "calibrationOperator",
        "calibrationDate",
        "dissipationTestPerformed",
        // Sondage / survey identity
        "sondageIdAccountableParty",
        "sondageId",
    ];
    if EXTRA_FIELDS.contains(&last) {
        let trimmed = txt.trim();
        if !trimmed.is_empty() {
            extras
                .entry(last.to_string())
                .and_modify(|prev| {
                    if !prev.contains(trimmed) {
                        prev.push_str(" | ");
                        prev.push_str(trimmed);
                    }
                })
                .or_insert_with(|| trimmed.to_string());
        }
    }

    match last {
        "broId" if id.is_none() => *id = Some(txt.to_string()),

        // Pick the RD `gml:pos` inside `deliveredLocation`, not the lat/long one
        // inside `standardizedLocation`.
        "pos" if path.iter().any(|p| p == "deliveredLocation") => {
            let nums: Vec<f64> = txt.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if nums.len() >= 2 {
                *x = Some(nums[0]);
                *y = Some(nums[1]);
            }
        }

        // Vertical reference offset (z-NAP) inside deliveredVerticalPosition.
        "offset" if path.iter().any(|p| p == "deliveredVerticalPosition") => {
            if let Ok(v) = txt.parse::<f64>() {
                *z = Some(v);
            }
        }

        // The date is in <researchReportDate><brocom:date>YYYY-MM-DD</brocom:date></...>.
        // We hit text at `date` whose ancestor is `researchReportDate`.
        "date" if path.iter().any(|p| p == "researchReportDate") => {
            if date.is_none() {
                if let Ok(d) = NaiveDate::parse_from_str(txt, "%Y-%m-%d") {
                    *date = Some(d);
                } else if let Ok(year) = txt.parse::<i32>() {
                    *date = NaiveDate::from_ymd_opt(year, 1, 1);
                }
            }
        }

        // Pick the 25-column data array from cptResult, NOT from dissipationTest.
        "values" if path.iter().any(|p| p == "cptResult") => {
            *data_block = Some(txt.to_string());
        }

        _ => {}
    }
}

fn parse_data_block(block: &str, z_nap: Option<f64>) -> Vec<MeasurementPoint> {
    // Records separated by ';', columns by ','
    block
        .split(';')
        .filter_map(|rec| {
            let trimmed = rec.trim();
            if trimmed.is_empty() {
                return None;
            }
            let nums: Vec<Option<f64>> = trimmed
                .split(',')
                .map(|s| {
                    let v = s.trim().parse::<f64>().ok()?;
                    if (v - VOID_VALUE).abs() < 0.5 {
                        None
                    } else {
                        Some(v)
                    }
                })
                .collect();
            if nums.len() < ORDER.len() {
                return None;
            }
            build_point(&nums, z_nap)
        })
        .collect()
}

fn build_point(nums: &[Option<f64>], z_nap: Option<f64>) -> Option<MeasurementPoint> {
    let mut p = MeasurementPoint {
        depth: 0.0,
        depth_nap: None,
        qc: None,
        fs: None,
        rf: None,
        u2: None,
        inclination: None,
    };
    let mut have_depth = false;

    for (i, field) in ORDER.iter().enumerate() {
        let v = nums[i];
        match field {
            BroField::Depth => {
                if let Some(d) = v {
                    p.depth = d;
                    have_depth = true;
                }
            }
            BroField::Length => {
                if !have_depth {
                    if let Some(d) = v {
                        p.depth = d;
                        have_depth = true;
                    }
                }
            }
            BroField::Qc => p.qc = v,
            BroField::Fs => p.fs = v,
            BroField::Rf => p.rf = v,
            BroField::U2 => p.u2 = v,
            BroField::Inclination => p.inclination = v,
            _ => {}
        }
    }
    if !have_depth {
        return None;
    }

    if p.rf.is_none() {
        if let (Some(qc), Some(fs)) = (p.qc, p.fs) {
            if qc > 0.0 {
                p.rf = Some(100.0 * fs / qc);
            }
        }
    }

    if let Some(z) = z_nap {
        p.depth_nap = Some(z - p.depth);
    }

    Some(p)
}
