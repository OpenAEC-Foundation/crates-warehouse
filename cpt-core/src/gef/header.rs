//! GEF header line parsing (`#KEY= value`).
//!
//! GEF is a line-oriented ASCII format. Header keywords start with `#`,
//! followed by `=` and a comma-or-whitespace separated value list.
//! `#EOH=` (end of header) marks the start of the data block.

use crate::error::CptError;
use super::columns::{from_quantity, GefField};

#[derive(Debug, Clone, Default)]
pub struct GefHeader {
    pub test_id: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub company_id: Option<String>,
    pub date: Option<chrono::NaiveDate>,
    pub x_rd: Option<f64>,
    pub y_rd: Option<f64>,
    pub z_nap: Option<f64>,
    pub columns: Vec<ColumnSpec>,
    pub column_void: Vec<(usize, f64)>, // (1-based column index, void value)
}

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub index: usize,        // 1-based GEF column index
    pub field: GefField,
}

pub fn parse_header(lines: &[&str]) -> Result<(GefHeader, usize), CptError> {
    let mut header = GefHeader::default();
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line == "#EOH=" || line == "#EOH" {
            return Ok((header, i + 1));
        }
        let Some(rest) = line.strip_prefix('#') else { continue };
        let Some((key, value)) = rest.split_once('=') else { continue };
        let key = key.trim().to_uppercase();
        let value = value.trim();
        match key.as_str() {
            "TESTID" => header.test_id = Some(value.to_string()),
            "PROJECTID" => header.project_id = Some(value.to_string()),
            "PROJECTNAME" => header.project_name = Some(value.to_string()),
            "COMPANYID" => header.company_id = Some(value.split(',').next().unwrap_or(value).trim().to_string()),
            "FILEDATE" => header.date = parse_filedate(value),
            "XYID" => parse_xyid(value, &mut header),
            "ZID" => parse_zid(value, &mut header),
            "COLUMNINFO" => parse_columninfo(value, &mut header),
            "COLUMNVOID" => parse_columnvoid(value, &mut header),
            _ => {} // ignore other keys for now
        }
    }
    Err(CptError::InvalidGef("missing #EOH terminator".into()))
}

fn parse_filedate(value: &str) -> Option<chrono::NaiveDate> {
    // FILEDATE format: "YYYY, MM, DD"
    let parts: Vec<i32> = value.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if parts.len() < 3 { return None; }
    chrono::NaiveDate::from_ymd_opt(parts[0], parts[1] as u32, parts[2] as u32)
}

fn parse_xyid(value: &str, h: &mut GefHeader) {
    // XYID format: "1, x, y, ..." — first field is coord-system id (1=RD), then x, y
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 { return; }
    if let (Ok(x), Ok(y)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
        h.x_rd = Some(x); h.y_rd = Some(y);
    }
}

fn parse_zid(value: &str, h: &mut GefHeader) {
    // ZID format: "31000, z, ..." — second field is z value (relative to NAP if id=31000)
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if parts.len() < 2 { return; }
    if let Ok(z) = parts[1].parse::<f64>() { h.z_nap = Some(z); }
}

fn parse_columninfo(value: &str, h: &mut GefHeader) {
    // COLUMNINFO format: "<col>, <unit>, <name>, <quantity>"
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if parts.len() < 4 { return; }
    let col: usize = match parts[0].parse() { Ok(n) => n, Err(_) => return };
    let q: u32 = match parts[3].parse() { Ok(n) => n, Err(_) => return };
    h.columns.push(ColumnSpec { index: col, field: from_quantity(q) });
}

fn parse_columnvoid(value: &str, h: &mut GefHeader) {
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if parts.len() < 2 { return; }
    if let (Ok(c), Ok(v)) = (parts[0].parse::<usize>(), parts[1].parse::<f64>()) {
        h.column_void.push((c, v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_header() {
        let lines = vec![
            "#GEFID= 1, 0, 0",
            "#TESTID= S01",
            "#PROJECTID= TEST-2026",
            "#XYID= 1, 100000.0, 400000.0",
            "#ZID= 31000, 2.5",
            "#COLUMN= 3",
            "#COLUMNINFO= 1, m, Sondeerlengte, 1",
            "#COLUMNINFO= 2, MPa, Conusweerstand, 2",
            "#COLUMNINFO= 3, MPa, Wrijving, 3",
            "#COLUMNVOID= 2, -9999",
            "#EOH=",
            "0.02 1.5 0.015",
        ];
        let (h, data_start) = parse_header(&lines).unwrap();
        assert_eq!(data_start, 11);
        assert_eq!(h.test_id.as_deref(), Some("S01"));
        assert_eq!(h.project_id.as_deref(), Some("TEST-2026"));
        assert_eq!(h.x_rd, Some(100_000.0));
        assert_eq!(h.y_rd, Some(400_000.0));
        assert_eq!(h.z_nap, Some(2.5));
        assert_eq!(h.columns.len(), 3);
        assert_eq!(h.columns[1].field, GefField::Qc);
        assert_eq!(h.column_void, vec![(2, -9999.0)]);
    }

    #[test]
    fn errors_on_missing_eoh() {
        let lines = vec!["#TESTID= S01", "0.02 1.5"];
        assert!(parse_header(&lines).is_err());
    }
}
