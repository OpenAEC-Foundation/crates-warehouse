use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use quick_xml::{events::Event, reader::Reader};

use crate::{BroError, CommonMetadata, Position, SchemaVersion, VerticalPosition};

#[derive(Debug)]
pub(crate) struct Leaf {
    pub(crate) path: String,
    pub(crate) value: String,
}

#[derive(Debug, Default)]
pub(crate) struct CollectedXml {
    pub(crate) leaves: Vec<Leaf>,
    attributes: BTreeMap<String, BTreeMap<String, String>>,
}

pub(crate) fn local_name(name: &[u8]) -> String {
    let local = name.rsplit(|byte| *byte == b':').next().unwrap_or(name);
    String::from_utf8_lossy(local).into_owned()
}

pub(crate) fn parse_f64(path: &str, value: &str) -> Result<f64, BroError> {
    value.parse().map_err(|_| BroError::InvalidValue {
        path: path.to_owned(),
        value: value.to_owned(),
    })
}

pub(crate) fn parse_date(path: &str, value: &str) -> Result<NaiveDate, BroError> {
    let date = value.get(..10).unwrap_or(value);
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| BroError::InvalidValue {
        path: path.to_owned(),
        value: value.to_owned(),
    })
}

pub(crate) fn required<'a>(value: Option<&'a str>, path: &str) -> Result<&'a str, BroError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| BroError::MissingField {
            path: path.to_owned(),
        })
}

pub(crate) fn collect(xml: &str) -> Result<CollectedXml, BroError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut collected = CollectedXml::default();
    let mut stack = Vec::new();
    let mut text = Vec::new();
    let mut has_child = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if let Some(parent_has_child) = has_child.last_mut() {
                    *parent_has_child = true;
                }
                stack.push(local_name(element.name().as_ref()));
                text.push(String::new());
                has_child.push(false);
                collect_attributes(&reader, &element, &stack.join("/"), &mut collected)?;
            }
            Ok(Event::Empty(element)) => {
                if let Some(parent_has_child) = has_child.last_mut() {
                    *parent_has_child = true;
                }
                stack.push(local_name(element.name().as_ref()));
                let path = stack.join("/");
                collect_attributes(&reader, &element, &path, &mut collected)?;
                collected.leaves.push(Leaf {
                    path,
                    value: String::new(),
                });
                stack.pop();
            }
            Ok(Event::Text(value)) => {
                if let Some(current_text) = text.last_mut() {
                    let decoded = value.unescape().map_err(|error| BroError::InvalidXml {
                        position: Some(reader.buffer_position()),
                        message: error.to_string(),
                    })?;
                    current_text.push_str(&decoded);
                }
            }
            Ok(Event::End(_)) => {
                let is_leaf = !has_child.pop().unwrap_or(true);
                let value = text.pop().unwrap_or_default().trim().to_owned();
                if is_leaf {
                    collected.leaves.push(Leaf {
                        path: stack.join("/"),
                        value,
                    });
                }
                stack.pop();
            }
            Ok(Event::Eof) => return Ok(collected),
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

fn collect_attributes(
    reader: &Reader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
    path: &str,
    collected: &mut CollectedXml,
) -> Result<(), BroError> {
    let mut attributes = BTreeMap::new();
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| BroError::InvalidXml {
            position: Some(reader.buffer_position()),
            message: error.to_string(),
        })?;
        let value = attribute
            .unescape_value()
            .map_err(|error| BroError::InvalidXml {
                position: Some(reader.buffer_position()),
                message: error.to_string(),
            })?;
        attributes.insert(local_name(attribute.key.as_ref()), value.into_owned());
    }
    if !attributes.is_empty() {
        collected.attributes.insert(path.to_owned(), attributes);
    }
    Ok(())
}

impl CollectedXml {
    pub(crate) fn value(&self, local: &str) -> Option<&str> {
        self.leaves
            .iter()
            .find(|leaf| leaf.path.rsplit('/').next() == Some(local))
            .map(|leaf| leaf.value.as_str())
    }

    fn ancestor_attribute(&self, path: &str, attribute: &str) -> Option<&str> {
        let mut ancestor = path;
        loop {
            if let Some(value) = self
                .attributes
                .get(ancestor)
                .and_then(|attributes| attributes.get(attribute))
            {
                return Some(value);
            }
            ancestor = ancestor.rsplit_once('/')?.0;
        }
    }
}

pub(crate) fn common_metadata(
    xml: &CollectedXml,
    schema_version: SchemaVersion,
) -> Result<CommonMetadata, BroError> {
    let bro_id = required(xml.value("broId"), "broId")?.to_owned();
    let position = parse_position(xml)?;
    let vertical_position = parse_vertical_position(xml)?;
    let registration_time = optional_date(xml, "objectRegistrationTime")?;
    let research_start_date = optional_date(xml, "researchStartDate")?;
    let research_end_date = optional_date(xml, "researchEndDate")?;
    let recognized = BTreeSet::from([
        "broId",
        "qualityRegime",
        "deliveryAccountableParty",
        "accountableParty",
        "objectRegistrationTime",
        "researchStartDate",
        "researchEndDate",
        "pos",
        "offset",
        "verticalDatum",
        "datum",
    ]);

    let extensions = xml
        .leaves
        .iter()
        .filter_map(|leaf| {
            let local = leaf.path.rsplit('/').next()?;
            (!recognized.contains(local)
                && leaf.value.len() < 200
                && !excluded_extension_path(&leaf.path))
            .then(|| (leaf.path.clone(), leaf.value.clone()))
        })
        .collect();

    Ok(CommonMetadata {
        bro_id,
        schema_version,
        quality_regime: xml.value("qualityRegime").map(str::to_owned),
        accountable_party: xml
            .value("deliveryAccountableParty")
            .or_else(|| xml.value("accountableParty"))
            .map(str::to_owned),
        registration_time,
        research_start_date,
        research_end_date,
        position,
        vertical_position,
        extensions,
    })
}

fn optional_date(xml: &CollectedXml, local: &str) -> Result<Option<NaiveDate>, BroError> {
    xml.value(local)
        .map(|value| parse_date(local, value))
        .transpose()
}

fn parse_position(xml: &CollectedXml) -> Result<Option<Position>, BroError> {
    let Some(leaf) = xml
        .leaves
        .iter()
        .find(|leaf| leaf.path.rsplit('/').next() == Some("pos"))
    else {
        return Ok(None);
    };
    let mut ordinates = leaf.value.split_whitespace();
    let x = parse_f64(&leaf.path, required(ordinates.next(), &leaf.path)?)?;
    let y = parse_f64(&leaf.path, required(ordinates.next(), &leaf.path)?)?;
    let crs_path = format!("{}/@srsName", leaf.path);
    let crs = required(xml.ancestor_attribute(&leaf.path, "srsName"), &crs_path)?.to_owned();
    Ok(Some(Position { x, y, crs }))
}

fn parse_vertical_position(xml: &CollectedXml) -> Result<Option<VerticalPosition>, BroError> {
    let Some(offset) = xml.value("offset") else {
        return Ok(None);
    };
    Ok(Some(VerticalPosition {
        offset: parse_f64("offset", offset)?,
        datum: xml
            .value("verticalDatum")
            .or_else(|| xml.value("datum"))
            .map(str::to_owned),
    }))
}

fn excluded_extension_path(path: &str) -> bool {
    path.split('/').any(|segment| {
        matches!(
            segment,
            "Point" | "pos" | "coordinates" | "values" | "elementValue" | "DataArray" | "result"
        )
    })
}
