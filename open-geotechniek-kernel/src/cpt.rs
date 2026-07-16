use std::collections::BTreeMap;

use bro_xml::{CommonMetadata, CptDocument};

pub(crate) fn from_bro(document: CptDocument, source_file: &str) -> cpt_core::Cpt {
    let CptDocument {
        common,
        final_depth,
        cone_type,
        measurements,
        ..
    } = document;
    let vertical_offset = common
        .vertical_position
        .as_ref()
        .map(|position| position.offset);
    let mut extra = common_extra(&common);
    if let Some(value) = cone_type {
        extra.insert("cone_type".to_owned(), value);
    }
    if let Some(value) = final_depth {
        extra.insert("final_depth".to_owned(), value.to_string());
    }

    let position = common.position.as_ref().and_then(|position| {
        if is_rd_crs(&position.crs) {
            Some(cpt_core::Position {
                x_rd: position.x,
                y_rd: position.y,
                z_nap: vertical_offset,
            })
        } else {
            extra.insert("position_x".to_owned(), position.x.to_string());
            extra.insert("position_y".to_owned(), position.y.to_string());
            None
        }
    });
    let points = measurements
        .into_iter()
        .map(|measurement| cpt_core::MeasurementPoint {
            depth: measurement.depth,
            depth_nap: vertical_offset.map(|offset| offset - measurement.depth),
            qc: measurement.cone_resistance,
            fs: measurement.sleeve_friction,
            rf: measurement.friction_ratio,
            u2: measurement.pore_pressure_u2,
            inclination: measurement.inclination,
        })
        .collect();

    cpt_core::Cpt {
        id: common.bro_id,
        metadata: cpt_core::Metadata {
            date: common.research_start_date,
            ground_level_nap: vertical_offset,
            source_file: source_file.to_owned(),
            extra,
            ..cpt_core::Metadata::default()
        },
        position,
        points,
    }
}

fn is_rd_crs(crs: &str) -> bool {
    let tokens = crs
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let Some(epsg_index) = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case("epsg"))
    else {
        return false;
    };
    let prefix = &tokens[..epsg_index];
    let suffix = &tokens[epsg_index + 1..];
    let recognized_authority = prefix.is_empty()
        || prefix
            .iter()
            .map(|token| token.to_ascii_lowercase())
            .collect::<Vec<_>>()
            == ["urn", "ogc", "def", "crs"]
        || (prefix.first().is_some_and(|token| {
            token.eq_ignore_ascii_case("http") || token.eq_ignore_ascii_case("https")
        }) && prefix
            .iter()
            .any(|token| token.eq_ignore_ascii_case("opengis"))
            && prefix
                .iter()
                .rev()
                .take(2)
                .map(|token| token.to_ascii_lowercase())
                .collect::<Vec<_>>()
                == ["crs", "def"]);
    recognized_authority
        && suffix.last().is_some_and(|code| *code == "28992")
        && suffix.len() <= 3
        && !suffix.is_empty()
        && suffix[..suffix.len() - 1]
            .iter()
            .all(|token| token.chars().all(|character| character.is_ascii_digit()))
}

fn common_extra(common: &CommonMetadata) -> BTreeMap<String, String> {
    let mut extra = common.extensions.clone();
    extra.insert(
        "schema_version".to_owned(),
        format!(
            "{}.{}",
            common.schema_version.major, common.schema_version.minor
        ),
    );
    insert_optional(
        &mut extra,
        "quality_regime",
        common.quality_regime.as_deref(),
    );
    insert_optional(
        &mut extra,
        "accountable_party",
        common.accountable_party.as_deref(),
    );
    insert_optional(
        &mut extra,
        "registration_time",
        common
            .registration_time
            .map(|value| value.to_string())
            .as_deref(),
    );
    insert_optional(
        &mut extra,
        "research_end_date",
        common
            .research_end_date
            .map(|value| value.to_string())
            .as_deref(),
    );
    if let Some(position) = &common.position {
        extra.insert("position_crs".to_owned(), position.crs.clone());
    }
    if let Some(datum) = common
        .vertical_position
        .as_ref()
        .and_then(|position| position.datum.as_deref())
    {
        extra.insert("vertical_datum".to_owned(), datum.to_owned());
    }
    extra
}

fn insert_optional(extra: &mut BTreeMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        extra.insert(key.to_owned(), value.to_owned());
    }
}
