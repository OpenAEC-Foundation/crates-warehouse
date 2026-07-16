use serde::{Deserialize, Serialize};

use crate::{detect, xml, BroDocumentType, BroError, CommonMetadata, ParseOptions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CptDocument {
    pub common: CommonMetadata,
    pub final_depth: Option<f64>,
    pub cone_type: Option<String>,
    pub measurements: Vec<CptMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CptMeasurement {
    pub depth: f64,
    pub cone_resistance: Option<f64>,
    pub sleeve_friction: Option<f64>,
    pub friction_ratio: Option<f64>,
    pub pore_pressure_u2: Option<f64>,
    pub inclination: Option<f64>,
}

pub(crate) fn parse(xml_source: &str, options: ParseOptions) -> Result<CptDocument, BroError> {
    let detected = detect(xml_source)?;
    if detected.document_type != BroDocumentType::Cpt {
        return Err(BroError::UnexpectedDocumentType {
            expected: BroDocumentType::Cpt,
            found: detected.document_type,
        });
    }
    let collected = xml::collect(xml_source)?;
    let common = xml::common_metadata(&collected, detected.schema_version)?;
    let values_path = collected.cpt_result_values_path();
    let values = xml::required(collected.cpt_result_values(), &values_path)?;
    let mut measurements = parse_measurements(values, &values_path)?;
    if measurements.is_empty() {
        return Err(BroError::MissingField { path: values_path });
    }
    measurements.sort_by(|left, right| left.depth.total_cmp(&right.depth));

    Ok(CptDocument {
        common,
        final_depth: collected
            .value("finalDepth")
            .map(|value| parse_optional_number(value, &collected.field_path("finalDepth")))
            .transpose()?
            .flatten(),
        cone_type: collected.value("coneType").map(str::to_owned),
        measurements,
        source_xml: options.retain_source.then(|| xml_source.to_owned()),
    })
}

fn parse_measurements(values: &str, path: &str) -> Result<Vec<CptMeasurement>, BroError> {
    const COLUMN_COUNT: usize = 25;
    const DEPTH: usize = 1;
    const CONE_RESISTANCE: usize = 3;
    const INCLINATION: usize = 15;
    const SLEEVE_FRICTION: usize = 18;
    const PORE_PRESSURE_U2: usize = 22;
    const FRICTION_RATIO: usize = 24;

    values
        .split(';')
        .filter(|row| !row.trim().is_empty())
        .enumerate()
        .map(|(row_index, row)| {
            let cells: Vec<_> = row.split(',').collect();
            if cells.len() != COLUMN_COUNT {
                return Err(BroError::InvalidValue {
                    path: format!("{path}/{row_index}"),
                    value: row.trim().to_owned(),
                });
            }

            let depth =
                parse_optional_cell(cells[DEPTH], path, row_index, DEPTH)?.ok_or_else(|| {
                    BroError::InvalidValue {
                        path: format!("{path}/{row_index}/{DEPTH}"),
                        value: cells[DEPTH].trim().to_owned(),
                    }
                })?;
            Ok(CptMeasurement {
                depth,
                cone_resistance: parse_optional_cell(
                    cells[CONE_RESISTANCE],
                    path,
                    row_index,
                    CONE_RESISTANCE,
                )?,
                sleeve_friction: parse_optional_cell(
                    cells[SLEEVE_FRICTION],
                    path,
                    row_index,
                    SLEEVE_FRICTION,
                )?,
                friction_ratio: parse_optional_cell(
                    cells[FRICTION_RATIO],
                    path,
                    row_index,
                    FRICTION_RATIO,
                )?,
                pore_pressure_u2: parse_optional_cell(
                    cells[PORE_PRESSURE_U2],
                    path,
                    row_index,
                    PORE_PRESSURE_U2,
                )?,
                inclination: parse_optional_cell(cells[INCLINATION], path, row_index, INCLINATION)?,
            })
        })
        .collect()
}

fn parse_optional_cell(
    cell: &str,
    path: &str,
    row_index: usize,
    column_index: usize,
) -> Result<Option<f64>, BroError> {
    let cell = cell.trim();
    parse_optional_number(cell, &format!("{path}/{row_index}/{column_index}"))
}

fn parse_optional_number(value: &str, path: &str) -> Result<Option<f64>, BroError> {
    const VOID_VALUE: f64 = -999_999.0;

    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let number = value.parse::<f64>().map_err(|_| BroError::InvalidValue {
        path: path.to_owned(),
        value: value.to_owned(),
    })?;
    Ok((number.is_finite() && number != VOID_VALUE).then_some(number))
}
