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

    let position = common.position.as_ref().map(|position| cpt_core::Position {
        x_rd: position.x,
        y_rd: position.y,
        z_nap: vertical_offset,
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
