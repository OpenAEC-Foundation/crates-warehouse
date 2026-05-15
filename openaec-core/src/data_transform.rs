//! Data transformation: API JSON → TemplateEngine flat dict.
//!
//! Port of `core/data_transform.py`. Transforms the nested JSON structure
//! (sections/content blocks) into the flat dot-notation format that the
//! template engine expects for bind-resolution.

use serde_json::{json, Map, Value};

/// Detect if the JSON data is already in flat engine format.
///
/// Flat format has top-level keys like 'bic', 'reiniging', 'samenvatting'
/// as objects (not lists or strings). The nested format has 'sections'.
fn is_already_flat(data: &Value) -> bool {
    let has_sections = data
        .get("sections")
        .is_some_and(|v| v.is_array());
    let has_flat_bic = data.get("bic").is_some_and(|v| v.is_object());
    let has_flat_meta = data.get("meta").is_some_and(|v| v.is_object());

    // If there is flat BIC/meta data AND no sections → already flat
    (has_flat_bic || has_flat_meta) && !has_sections
}

/// Transform API JSON to the flat format that the TemplateEngine expects.
///
/// If the data is already flat (contains top-level 'bic', 'meta', etc. dicts
/// without 'sections' list), the data is passed through unchanged.
pub fn transform_json_to_engine_data(raw: &Value) -> Value {
    if is_already_flat(raw) {
        return raw.clone();
    }
    transform_nested(raw)
}

/// Transform nested sections-structure to flat engine format.
fn transform_nested(raw: &Value) -> Value {
    // Cover extra fields
    let cover = raw.get("cover").and_then(|c| c.as_object());
    let extra = cover
        .and_then(|c| c.get("extra_fields"))
        .and_then(|e| e.as_object());

    // Client + Location data from sections
    let mut client_data = json!({
        "name": raw.get("client").and_then(|c| c.as_str()).unwrap_or(""),
        "address": "",
        "postcode_plaats": "",
    });
    let mut location_data = json!({});

    // BIC data — per section
    let mut bic_data = Map::new();
    let mut reiniging_data = Map::new();
    let mut additioneel_data = Map::new();
    let mut samenvatting_data = Map::new();

    let mut detail_items: Vec<Value> = Vec::new();
    let mut objecten: Vec<Value> = Vec::new();

    if let Some(sections) = raw.get("sections").and_then(|s| s.as_array()) {
        for section in sections {
            let section_title = section
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            let content = section.get("content").and_then(|c| c.as_array());
            let Some(content) = content else { continue };

            for block in content {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match block_type {
                    "location_detail" => {
                        let cl = block.get("client").and_then(|c| c.as_object());
                        let loc = block.get("location").and_then(|l| l.as_object());

                        if let Some(cl) = cl {
                            let default_name = raw
                                .get("client")
                                .and_then(|c| c.as_str())
                                .unwrap_or("");
                            client_data = json!({
                                "name": cl.get("name").and_then(|v| v.as_str()).unwrap_or(default_name),
                                "address": cl.get("address").and_then(|v| v.as_str()).unwrap_or(""),
                                "postcode_plaats": cl.get("city").and_then(|v| v.as_str()).unwrap_or(""),
                            });
                        }
                        if let Some(loc) = loc {
                            location_data = json!({
                                "name": get_str(loc, "name"),
                                "address": get_str(loc, "address"),
                                "postcode_plaats": get_str(loc, "city"),
                                "code": get_str(loc, "code"),
                                "provision": get_str(loc, "provision"),
                                "object": get_str(loc, "object"),
                            });
                        }
                    }
                    "bic_table" => {
                        if let Some(bic_sections) =
                            block.get("sections").and_then(|s| s.as_array())
                        {
                            let (b, r, a) = parse_bic_sections(bic_sections);
                            bic_data = b;
                            reiniging_data = r;
                            additioneel_data = a;
                        }
                        if let Some(summary) =
                            block.get("summary").and_then(|s| s.as_object())
                        {
                            samenvatting_data = parse_bic_summary(summary);
                        }
                    }
                    "table" => {
                        let headers: Vec<&str> = block
                            .get("headers")
                            .and_then(|h| h.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect()
                            })
                            .unwrap_or_default();
                        let rows = block.get("rows").and_then(|r| r.as_array());

                        if section_title.contains("Detail") {
                            if let Some(rows) = rows {
                                for row in rows {
                                    if let Some(row_arr) = row.as_array() {
                                        let mut item = Map::new();
                                        for (i, v) in row_arr.iter().enumerate() {
                                            let key = if i < headers.len() {
                                                headers[i].to_string()
                                            } else {
                                                format!("col_{i}")
                                            };
                                            item.insert(key, v.clone());
                                        }
                                        detail_items.push(Value::Object(item));
                                    }
                                }
                            }
                        } else if (section_title.to_lowercase().contains("objecten")
                            || section_title.contains("Voorziening"))
                            && let Some(rows) = rows {
                                for row in rows {
                                    if let Some(row_arr) = row.as_array() {
                                        let mut item = Map::new();
                                        let mut type_count = 0u32;
                                        for (i, v) in row_arr.iter().enumerate() {
                                            let mut key = if i < headers.len() {
                                                headers[i].to_string()
                                            } else {
                                                format!("col_{i}")
                                            };
                                            if key == "Type" {
                                                type_count += 1;
                                                if type_count > 1 {
                                                    key = "Type2".to_string();
                                                }
                                            }
                                            item.insert(key, v.clone());
                                        }
                                        objecten.push(Value::Object(item));
                                    }
                                }
                            }
                    }
                    _ => {}
                }
            }
        }
    }

    // Compose derived fields
    let type_offerte = extra_str(extra, "Type offerte");
    let offerte_code = extra_str(extra, "Offertecode")
        .or_else(|| raw.get("project_number").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_default();
    let offerte_naam = extra_str(extra, "Offertenaam").unwrap_or_default();
    let offerte_regel = if offerte_naam.is_empty() {
        offerte_code.clone()
    } else {
        format!("{offerte_code}: {offerte_naam}")
    };

    let report_type = raw
        .get("report_type")
        .and_then(|v| v.as_str())
        .unwrap_or("BIC Factuur");
    let loc_code = location_data
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let rapportkop_locatie = if loc_code.is_empty() {
        report_type.to_string()
    } else {
        format!("{report_type}: {loc_code}")
    };

    let datum = extra_str(extra, "Datum")
        .or_else(|| raw.get("date").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_default();
    let factuurnummer = extra_str(extra, "Factuurnummer").unwrap_or_default();
    let type_offerte_display = if type_offerte.as_deref().unwrap_or("").is_empty() {
        String::new()
    } else {
        format!("{}:", type_offerte.unwrap())
    };

    json!({
        "meta": {
            "factuur_kop": report_type,
            "datum": datum,
            "factuurnummer": factuurnummer,
            "type_offerte": type_offerte_display,
            "offerte_regel": offerte_regel,
            "rapportkop_locatie": rapportkop_locatie,
        },
        "project": {
            "name": raw.get("project").and_then(|v| v.as_str()).unwrap_or(""),
        },
        "client": client_data,
        "location": location_data,
        "bic": Value::Object(bic_data),
        "reiniging": Value::Object(reiniging_data),
        "additioneel": Value::Object(additioneel_data),
        "samenvatting": Value::Object(samenvatting_data),
        "detail_items": detail_items,
        "objecten": objecten,
    })
}

/// Parse BIC table sections into flat dicts per category.
fn parse_bic_sections(sections: &[Value]) -> (Map<String, Value>, Map<String, Value>, Map<String, Value>) {
    let mut bic = Map::new();
    let mut reiniging = Map::new();
    let mut additioneel = Map::new();

    for sec in sections {
        let title = sec
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_lowercase();
        let rows = sec.get("rows").and_then(|r| r.as_array());
        let Some(rows) = rows else { continue };

        if title.contains("bic controle") {
            map_bic_rows(rows, &mut bic);
        } else if title.contains("reinig") {
            map_simple_rows(rows, &mut reiniging);
        } else if title.contains("additioneel") {
            map_simple_rows(rows, &mut additioneel);
        }
    }

    (bic, reiniging, additioneel)
}

/// Map BIC controle rows to flat keys.
fn map_bic_rows(rows: &[Value], out: &mut Map<String, Value>) {
    let mut kosten_idx = 0u32;
    for row in rows {
        let label = row
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_lowercase();
        let ref_val = row_value(row, "ref_value");
        let actual = row_value(row, "actual_value");

        if label.contains("aantal bic") {
            out.insert("aantal_conform".into(), ref_val);
            out.insert("aantal_werkelijk".into(), actual);
        } else if label.contains("aantal interne") || label.contains("aantal hydro") {
            out.insert("hydro_aantal_conform".into(), ref_val);
            out.insert("hydro_aantal_werkelijk".into(), actual);
        } else if label.contains("reiskosten") {
            out.insert("reiskosten_conform".into(), ref_val);
            out.insert("reiskosten_werkelijk".into(), actual);
        } else if label.contains("subtotaal") {
            out.insert("subtotaal_conform".into(), ref_val);
            out.insert("subtotaal_werkelijk".into(), actual);
        } else if label.contains("kosten") {
            kosten_idx += 1;
            if kosten_idx == 1 {
                out.insert("kosten_conform".into(), ref_val);
                out.insert("kosten_werkelijk".into(), actual);
            } else {
                out.insert("hydro_kosten_conform".into(), ref_val);
                out.insert("hydro_kosten_werkelijk".into(), actual);
            }
        }
    }
}

/// Map simple section rows (reiniging/additioneel).
fn map_simple_rows(rows: &[Value], out: &mut Map<String, Value>) {
    for row in rows {
        let label = row
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_lowercase();
        let ref_val = row_value(row, "ref_value");
        let actual = row_value(row, "actual_value");

        if label.contains("aantal") {
            out.insert("aantal_conform".into(), ref_val);
            out.insert("aantal_werkelijk".into(), actual);
        } else if label.contains("kosten") {
            out.insert("kosten_conform".into(), ref_val);
            out.insert("kosten_werkelijk".into(), actual);
        }
    }
}

/// Parse BIC summary section.
fn parse_bic_summary(summary: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();

    if let Some(rows) = summary.get("rows").and_then(|r| r.as_array()) {
        for row in rows {
            let label = row
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .to_lowercase();
            let ref_val = row_value(row, "ref_value");
            let actual = row_value(row, "actual_value");

            if label.contains("bic") {
                out.insert("bic_conform".into(), ref_val);
                out.insert("bic_werkelijk".into(), actual);
            } else if label.contains("reinig") {
                out.insert("reinigen_conform".into(), ref_val);
                out.insert("reinigen_werkelijk".into(), actual);
            } else if label.contains("additioneel") {
                out.insert("additioneel_conform".into(), ref_val);
                out.insert("additioneel_werkelijk".into(), actual);
            }
        }
    }

    if let Some(total) = summary.get("total").and_then(|t| t.as_object()) {
        out.insert(
            "totaal_conform".into(),
            row_value(&Value::Object(total.clone()), "ref_value"),
        );
        out.insert(
            "totaal_werkelijk".into(),
            row_value(&Value::Object(total.clone()), "actual_value"),
        );
    }

    out
}

// ── BIC inject helpers ────────────────────────────────────────────────

/// Static TOC data for BIC Rapport templates.
fn bic_rapport_toc() -> Value {
    json!({
        "item_1": "1  Locatie",
        "item_2": "2  Voorziening",
        "item_3": "3  Object",
        "item_4": "4  Bedrijfsinterne controle (BIC)",
        "item_5": "5  Herstelwerkzaamheden",
        "item_6": "6  Tekeningen",
        "item_6_1": "6.1  Regionale overzichtstekening",
        "item_6_2": "6.2  Detailtekening",
        "item_6_3": "6.3  Kadastrale kaart",
        "item_7": "7  Onderhoudsdossier",
        "item_7_1": "7.1  Controlelijst BIC",
        "item_7_2": "7.2  Historie",
        "item_7_2_1": "7.2.1  BIC controles",
        "item_7_2_2": "7.2.2  Herstelwerkzaamheden",
        "item_7_2_3": "7.2.3  Onderhouds- en inspectieoverzicht",
        "item_8": "8  Bijlagen",
        "item_8_1": "8.1  Fotobijlage",
        "item_8_2": "8.2  Certificaten",
        "item_8_3": "8.3  Overige documenten",
    })
}

/// Inject static TOC data for BIC Rapport templates.
///
/// BIC Rapporten have a fixed table of contents. The template defines
/// text_zones (toc.item_1 through toc.item_8_3) but the data must also
/// contain the corresponding values.
///
/// Skips if a `toc` dict with `item_*` keys is already present.
pub fn inject_toc_if_needed(result: &mut Value, raw: &Value) {
    let template = raw
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !template.contains("bic_rapport") {
        return;
    }

    // Skip if toc already has item_* keys
    if let Some(toc) = result.get("toc").and_then(|v| v.as_object()) {
        if toc.keys().any(|k| k.starts_with("item_")) {
            return;
        }
    }

    result
        .as_object_mut()
        .expect("result must be an object")
        .insert("toc".into(), bic_rapport_toc());
}

/// Add dynamic labels to location dict if corresponding values exist.
///
/// The location page_type binds on `location.provision_label` and
/// `location.object_label`. These are only shown when the corresponding
/// value is non-empty.
pub fn inject_location_labels(result: &mut Value) {
    let loc = match result.get_mut("location").and_then(|v| v.as_object_mut()) {
        Some(loc) => loc,
        None => return,
    };

    let has_provision = loc
        .get("provision")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());
    let has_provision_label = loc.contains_key("provision_label");

    if !has_provision_label && has_provision {
        loc.insert("provision_label".into(), json!("Voorziening"));
    }

    let has_object = loc
        .get("object")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());
    let has_object_label = loc.contains_key("object_label");

    if !has_object_label && has_object {
        loc.insert("object_label".into(), json!("Object"));
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn get_str(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn extra_str(extra: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    extra
        .and_then(|e| e.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn row_value(row: &Value, key: &str) -> Value {
    row.get(key)
        .cloned()
        .unwrap_or(Value::String(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_already_flat_data() {
        let data = json!({
            "bic": {"aantal_conform": "10"},
            "meta": {"datum": "2026-01-01"},
        });
        assert!(is_already_flat(&data));
    }

    #[test]
    fn test_nested_data_not_flat() {
        let data = json!({
            "sections": [{"title": "Test", "content": []}],
            "project": "Test",
        });
        assert!(!is_already_flat(&data));
    }

    #[test]
    fn test_passthrough_flat_data() {
        let data = json!({
            "bic": {"aantal_conform": "10"},
            "meta": {"datum": "2026-01-01"},
        });
        let result = transform_json_to_engine_data(&data);
        assert_eq!(result, data);
    }

    #[test]
    fn test_transform_nested_basic() {
        let data = json!({
            "project": "Test Project",
            "client": "Test Client",
            "date": "2026-03-15",
            "report_type": "BIC Factuur",
            "sections": [],
        });
        let result = transform_json_to_engine_data(&data);
        assert_eq!(result["project"]["name"], "Test Project");
        assert_eq!(result["client"]["name"], "Test Client");
        assert_eq!(result["meta"]["datum"], "2026-03-15");
    }

    #[test]
    fn test_transform_location_detail() {
        let data = json!({
            "project": "P",
            "sections": [{
                "title": "Locatie",
                "content": [{
                    "type": "location_detail",
                    "client": {"name": "Klant A", "address": "Straat 1", "city": "Amsterdam"},
                    "location": {"name": "Loc X", "code": "L001"},
                }],
            }],
        });
        let result = transform_json_to_engine_data(&data);
        assert_eq!(result["client"]["name"], "Klant A");
        assert_eq!(result["location"]["code"], "L001");
    }

    #[test]
    fn test_transform_bic_table() {
        let data = json!({
            "project": "P",
            "sections": [{
                "title": "BIC",
                "content": [{
                    "type": "bic_table",
                    "sections": [{
                        "title": "BIC controles",
                        "rows": [
                            {"label": "Aantal BIC controles", "ref_value": "10", "actual_value": "8"},
                            {"label": "Kosten", "ref_value": "500", "actual_value": "400"},
                        ],
                    }],
                    "summary": {
                        "rows": [
                            {"label": "BIC controles", "ref_value": "500", "actual_value": "400"},
                        ],
                        "total": {"ref_value": "1000", "actual_value": "800"},
                    },
                }],
            }],
        });
        let result = transform_json_to_engine_data(&data);
        assert_eq!(result["bic"]["aantal_conform"], "10");
        assert_eq!(result["bic"]["kosten_werkelijk"], "400");
        assert_eq!(result["samenvatting"]["totaal_conform"], "1000");
    }

    #[test]
    fn test_inject_toc_if_needed_bic_rapport() {
        let raw = json!({"template": "bic_rapport"});
        let mut result = json!({"meta": {"datum": "2026-01-01"}});
        inject_toc_if_needed(&mut result, &raw);
        assert_eq!(result["toc"]["item_1"], "1  Locatie");
        assert_eq!(result["toc"]["item_8_3"], "8.3  Overige documenten");
    }

    #[test]
    fn test_inject_toc_skips_non_bic() {
        let raw = json!({"template": "structural"});
        let mut result = json!({});
        inject_toc_if_needed(&mut result, &raw);
        assert!(result.get("toc").is_none());
    }

    #[test]
    fn test_inject_toc_skips_existing() {
        let raw = json!({"template": "bic_rapport"});
        let mut result = json!({"toc": {"item_1": "Custom"}});
        inject_toc_if_needed(&mut result, &raw);
        // Should NOT overwrite existing toc
        assert_eq!(result["toc"]["item_1"], "Custom");
    }

    #[test]
    fn test_inject_location_labels() {
        let mut result = json!({
            "location": {
                "provision": "Gasleiding",
                "object": "Hoofdleiding",
            }
        });
        inject_location_labels(&mut result);
        assert_eq!(result["location"]["provision_label"], "Voorziening");
        assert_eq!(result["location"]["object_label"], "Object");
    }

    #[test]
    fn test_inject_location_labels_skips_empty() {
        let mut result = json!({
            "location": {
                "provision": "",
                "object": "",
            }
        });
        inject_location_labels(&mut result);
        assert!(result["location"].get("provision_label").is_none());
        assert!(result["location"].get("object_label").is_none());
    }

    #[test]
    fn test_inject_location_labels_preserves_existing() {
        let mut result = json!({
            "location": {
                "provision": "Gasleiding",
                "provision_label": "Custom Label",
            }
        });
        inject_location_labels(&mut result);
        assert_eq!(result["location"]["provision_label"], "Custom Label");
    }

    #[test]
    fn test_map_bic_rows_kosten_index() {
        let rows = vec![
            json!({"label": "Kosten BIC", "ref_value": "100", "actual_value": "80"}),
            json!({"label": "Kosten hydro", "ref_value": "200", "actual_value": "150"}),
        ];
        let mut out = Map::new();
        map_bic_rows(&rows, &mut out);
        assert_eq!(out["kosten_conform"], "100");
        assert_eq!(out["hydro_kosten_conform"], "200");
    }
}
