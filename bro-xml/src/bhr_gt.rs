use quick_xml::{events::Event, reader::Reader};
use serde::{Deserialize, Serialize};

use crate::{detect, xml, BroDocumentType, BroError, CommonMetadata, ParseOptions};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BhrGtDocument {
    pub common: CommonMetadata,
    pub final_depth: Option<f64>,
    pub boring_procedure: Option<String>,
    pub description_procedure: Option<String>,
    pub intervals: Vec<GeotechnicalInterval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeotechnicalInterval {
    pub upper_boundary: f64,
    pub lower_boundary: f64,
    pub soil_name: Option<String>,
    pub colour: Option<String>,
    pub description: Option<String>,
    pub secondary: Vec<SecondaryAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondaryAttribute {
    pub code: String,
    pub value: String,
}

pub(crate) fn parse(xml_source: &str, options: ParseOptions) -> Result<BhrGtDocument, BroError> {
    let detected = detect(xml_source)?;
    if detected.document_type != BroDocumentType::BhrGt {
        return Err(BroError::UnexpectedDocumentType {
            expected: BroDocumentType::BhrGt,
            found: detected.document_type,
        });
    }
    let collected = xml::collect(xml_source)?;
    let common = xml::common_metadata(&collected, detected.schema_version)?;
    require_element(xml_source, "boring", &collected.field_path("boring"))?;
    let mut intervals = parse_intervals(xml_source)?;
    intervals.sort_by(|left, right| left.upper_boundary.total_cmp(&right.upper_boundary));
    intervals.dedup();

    Ok(BhrGtDocument {
        common,
        final_depth: optional_finite_number(&collected, "finalDepthBoring")?,
        boring_procedure: collected.value("boringProcedure").map(str::to_owned),
        description_procedure: collected.value("descriptionProcedure").map(str::to_owned),
        intervals,
        source_xml: options.retain_source.then(|| xml_source.to_owned()),
    })
}

fn require_element(xml_source: &str, local: &str, path: &str) -> Result<(), BroError> {
    let mut reader = Reader::from_str(xml_source);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element))
                if xml::local_name(element.name().as_ref()) == local =>
            {
                return Ok(())
            }
            Ok(Event::Eof) => {
                return Err(BroError::MissingField {
                    path: path.to_owned(),
                })
            }
            Ok(_) => {}
            Err(error) => {
                return Err(BroError::InvalidXml {
                    position: Some(reader.buffer_position()),
                    message: error.to_string(),
                })
            }
        }
    }
}

#[derive(Debug)]
struct LeafValue {
    relative_path: String,
    absolute_path: String,
    value: String,
}

#[derive(Debug)]
struct LayerBuilder {
    depth: usize,
    path: String,
    leaves: Vec<LeafValue>,
}

fn parse_intervals(xml_source: &str) -> Result<Vec<GeotechnicalInterval>, BroError> {
    let mut reader = Reader::from_str(xml_source);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut text = Vec::<String>::new();
    let mut builders = Vec::<LayerBuilder>::new();
    let mut completed = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let local = xml::local_name(element.name().as_ref());
                stack.push(local.clone());
                text.push(String::new());
                if is_layer_name(&local) {
                    builders.push(LayerBuilder {
                        depth: stack.len(),
                        path: stack.join("/"),
                        leaves: Vec::new(),
                    });
                }
            }
            Ok(Event::Text(value)) => {
                if let Some(current) = text.last_mut() {
                    let decoded = value.unescape().map_err(|error| BroError::InvalidXml {
                        position: Some(reader.buffer_position()),
                        message: error.to_string(),
                    })?;
                    current.push_str(&decoded);
                }
            }
            Ok(Event::End(_)) => {
                let value = text.pop().unwrap_or_default().trim().to_owned();
                if !value.is_empty() {
                    let absolute_path = stack.join("/");
                    for builder in &mut builders {
                        if stack.len() > builder.depth {
                            builder.leaves.push(LeafValue {
                                relative_path: stack[builder.depth..].join("/"),
                                absolute_path: absolute_path.clone(),
                                value: value.clone(),
                            });
                        }
                    }
                }

                if stack.last().is_some_and(|local| is_layer_name(local)) {
                    let depth = stack.len();
                    if let Some(index) = builders.iter().rposition(|item| item.depth == depth) {
                        let builder = builders.remove(index);
                        if let Some(interval) = build_interval(builder)? {
                            completed.push(interval);
                        }
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => return Ok(completed),
            Ok(_) => {}
            Err(error) => {
                return Err(BroError::InvalidXml {
                    position: Some(reader.buffer_position()),
                    message: error.to_string(),
                });
            }
        }
    }
}

fn build_interval(builder: LayerBuilder) -> Result<Option<GeotechnicalInterval>, BroError> {
    let upper = field(&builder.leaves, &["upperboundary"]);
    let lower = field(&builder.leaves, &["lowerboundary"]);
    if upper.is_none() && lower.is_none() {
        return Ok(None);
    }
    let upper = required_boundary(upper, &format!("{}/upperBoundary", builder.path))?;
    let lower_path = format!("{}/lowerBoundary", builder.path);
    let lower = required_boundary(lower, &lower_path)?;
    if lower.0 <= upper.0 {
        return Err(BroError::InvalidValue {
            path: lower.1,
            value: lower.0.to_string(),
        });
    }

    const SECONDARY_FIELDS: &[&str] = &[
        "anomalouslayer",
        "chunks",
        "peatfraction",
        "pedologicalsoilname",
        "organicmattercontentclass",
        "carbonatecontentclass",
        "ripening",
        "structure",
        "soilstructure",
        "horizon",
        "horizonvalue",
        "soilhorizon",
    ];
    let mut secondary = Vec::new();
    for leaf in &builder.leaves {
        if let Some(code) = matching_segment(&leaf.relative_path, SECONDARY_FIELDS) {
            let attribute = SecondaryAttribute {
                code: code.to_owned(),
                value: leaf.value.clone(),
            };
            if !secondary.contains(&attribute) {
                secondary.push(attribute);
            }
        }
    }

    Ok(Some(GeotechnicalInterval {
        upper_boundary: upper.0,
        lower_boundary: lower.0,
        soil_name: field_value(&builder.leaves, &["geotechnicalsoilname", "soilname"]),
        colour: field_value(&builder.leaves, &["colour", "color"]),
        description: field_value(&builder.leaves, &["description"]),
        secondary,
    }))
}

fn is_layer_name(local: &str) -> bool {
    matches!(
        normalize(local).as_str(),
        "layer" | "geotechnicalinterval" | "interval"
    )
}

fn field<'a>(leaves: &'a [LeafValue], names: &[&str]) -> Option<&'a LeafValue> {
    leaves
        .iter()
        .find(|leaf| matching_segment(&leaf.relative_path, names).is_some())
}

fn field_value(leaves: &[LeafValue], names: &[&str]) -> Option<String> {
    field(leaves, names).map(|leaf| leaf.value.clone())
}

fn matching_segment<'a>(path: &'a str, names: &[&str]) -> Option<&'a str> {
    path.split('/')
        .find(|segment| names.contains(&normalize(segment).as_str()))
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn required_boundary(
    leaf: Option<&LeafValue>,
    fallback_path: &str,
) -> Result<(f64, String), BroError> {
    let leaf = leaf.ok_or_else(|| BroError::MissingField {
        path: fallback_path.to_owned(),
    })?;
    let number = xml::parse_f64(&leaf.absolute_path, leaf.value.trim())?;
    if !number.is_finite() {
        return Err(BroError::InvalidValue {
            path: leaf.absolute_path.clone(),
            value: leaf.value.clone(),
        });
    }
    Ok((number, leaf.absolute_path.clone()))
}

fn optional_finite_number(
    collected: &xml::CollectedXml,
    local: &str,
) -> Result<Option<f64>, BroError> {
    let Some(value) = collected.value(local) else {
        return Ok(None);
    };
    let path = collected.field_path(local);
    let number = xml::parse_f64(&path, value.trim())?;
    if !number.is_finite() {
        return Ok(None);
    }
    Ok(Some(number))
}
