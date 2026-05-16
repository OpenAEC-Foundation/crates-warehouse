//! GEF data section parsing.
//!
//! After `#EOH=`, lines contain numeric values one row per measurement.
//! Default separator: whitespace. Custom separator via `#COLUMNSEPARATOR= X`.
//! Default record separator: newline. Custom via `#RECORDSEPARATOR= Y`.

use crate::domain::{Cpt, MeasurementPoint, Metadata, Position};
use crate::error::CptError;
use super::columns::GefField;
use super::header::{parse_header, GefHeader};

pub fn parse(text: &str) -> Result<Cpt, CptError> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let (header, data_start) = parse_header(&lines)?;

    let (col_sep, rec_sep) = extract_separators(&lines);
    let body: String = lines[data_start..].join("\n");

    // Tokenize into records
    let records: Vec<String> = if let Some(rs) = rec_sep {
        body.split(rs).map(|s| s.to_string()).collect()
    } else {
        body.lines().map(|s| s.to_string()).collect()
    };

    let mut points = Vec::new();
    for rec in records {
        let trimmed = rec.trim();
        if trimmed.is_empty() { continue; }
        let nums: Vec<f64> = if let Some(cs) = col_sep {
            trimmed.split(cs).filter_map(parse_num).collect()
        } else {
            trimmed.split_whitespace().filter_map(parse_num).collect()
        };
        if nums.is_empty() { continue; }
        if let Some(pt) = build_point(&nums, &header) {
            points.push(pt);
        }
    }

    let position = match (header.x_rd, header.y_rd) {
        (Some(x), Some(y)) => Some(Position { x_rd: x, y_rd: y, z_nap: header.z_nap }),
        _ => None,
    };

    Ok(Cpt {
        id: header.test_id.clone().unwrap_or_else(|| "Unknown".into()),
        metadata: Metadata {
            project_name: header.project_name.clone(),
            project_number: header.project_id.clone(),
            // Prefer the actual measurement date (#STARTDATE) over the file
            // write date (#FILEDATE) — matches bedrock-engineer/gef-parser-ts
            // and is what shows up in field reports.
            date: header.start_date.or(header.date),
            equipment: header.company_id.clone(),
            ground_level_nap: header.z_nap,
            source_file: String::new(),
            extra: header.extra.clone(),
        },
        position,
        points,
    })
}

fn parse_num(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() { return None; }
    s.parse::<f64>().ok()
}

fn extract_separators(lines: &[&str]) -> (Option<char>, Option<char>) {
    let mut col = None;
    let mut rec = None;
    for raw in lines {
        let line = raw.trim();
        if let Some(v) = line.strip_prefix("#COLUMNSEPARATOR=").map(str::trim) {
            col = v.chars().next();
        } else if let Some(v) = line.strip_prefix("#RECORDSEPARATOR=").map(str::trim) {
            rec = v.chars().next();
        }
    }
    (col, rec)
}

fn build_point(nums: &[f64], header: &GefHeader) -> Option<MeasurementPoint> {
    let mut p = MeasurementPoint {
        depth: 0.0,
        depth_nap: None,
        qc: None, fs: None, rf: None, u2: None, inclination: None,
    };
    let mut have_depth = false;

    for spec in &header.columns {
        let raw = nums.get(spec.index - 1).copied()?;
        // Apply void filter
        let voided = header.column_void.iter().any(|(c, v)| *c == spec.index && (raw - v).abs() < 1e-6);
        let value = if voided { None } else { Some(raw) };

        match spec.field {
            GefField::Length | GefField::Depth => {
                // Normalize sign — some GEFs (especially Belgian flavor) record
                // depth as a negative value below ground. Mirror bedrock-engineer's
                // gef-parser-ts behavior: take the absolute value so downstream
                // code can rely on "positive depth = below ground".
                if let Some(v) = value { p.depth = v.abs(); have_depth = true; }
            }
            GefField::Qc => p.qc = value,
            GefField::Fs => p.fs = value,
            GefField::Rf => p.rf = value,
            GefField::U2 => p.u2 = value,
            GefField::Inclination => p.inclination = value,
            _ => {}
        }
    }

    if !have_depth { return None; }

    // Derive Rf from qc + fs if missing
    if p.rf.is_none() {
        if let (Some(qc), Some(fs)) = (p.qc, p.fs) {
            if qc > 0.0 { p.rf = Some(100.0 * fs / qc); }
        }
    }

    if let Some(z) = header.z_nap {
        p.depth_nap = Some(z - p.depth);
    }

    Some(p)
}
