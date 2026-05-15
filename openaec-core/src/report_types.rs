//! Report type builders — structural, daylight, building code.
//!
//! Port of `reports/structural.py`, `reports/daylight.py`, `reports/building_code.py`.
//! Defines a `ReportBuilder` trait and concrete implementations for each report type.
//! These builders transform domain-specific data into generic sections with content blocks.

use serde_json::Value;

use crate::schema::{
    CalculationBlock as CalcSchema, CheckBlock as CheckSchema, CheckResult, ContentBlock,
    ParagraphBlock, Section, TableBlock as TableSchema,
};

// ── ReportBuilder trait ────────────────────────────────────────────────

/// Trait for report type builders.
///
/// Each report type transforms its domain-specific data structure into
/// a list of generic `Section`s containing `ContentBlock`s.
pub trait ReportBuilder: Send + Sync {
    /// Report type identifier (e.g., "structural", "daylight").
    fn report_type(&self) -> &str;

    /// Default template name for this report type.
    fn default_template(&self) -> &str;

    /// Build sections from the provided data.
    fn build_sections(&self, data: &Value) -> Vec<Section>;
}

/// Get a report builder by type name.
pub fn get_builder(report_type: &str) -> Option<Box<dyn ReportBuilder>> {
    match report_type {
        "structural" => Some(Box::new(StructuralReport)),
        "daylight" => Some(Box::new(DaylightReport)),
        "building_code" => Some(Box::new(BuildingCodeReport)),
        _ => None,
    }
}

// ── Structural Report ──────────────────────────────────────────────────

/// Constructieve berekening rapport.
pub struct StructuralReport;

impl ReportBuilder for StructuralReport {
    fn report_type(&self) -> &str {
        "structural"
    }

    fn default_template(&self) -> &str {
        "structural_report"
    }

    fn build_sections(&self, data: &Value) -> Vec<Section> {
        let mut sections = Vec::new();
        sections.extend(build_uitgangspunten(data));
        sections.extend(build_belastingen(data));
        sections.extend(build_elementen(data));
        sections.extend(build_uc_overzicht(data));
        sections.extend(build_conclusie(data));
        sections
    }
}

fn build_uitgangspunten(data: &Value) -> Vec<Section> {
    let uitg = match data.get("uitgangspunten") {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };

    let mut content: Vec<ContentBlock> = Vec::new();

    if let Some(beschr) = uitg.get("beschrijving").and_then(|v| v.as_str())
        && !beschr.is_empty() {
            content.push(paragraph(beschr));
        }

    if let Some(normen) = uitg.get("normen").and_then(|v| v.as_array()) {
        let normen_list: Vec<&str> = normen.iter().filter_map(|n| n.as_str()).collect();
        if !normen_list.is_empty() {
            content.push(paragraph(&format!(
                "Toegepaste normen: {}.",
                normen_list.join(", ")
            )));
        }
    }

    if let Some(materialen) = uitg.get("materialen").and_then(|v| v.as_array())
        && !materialen.is_empty() {
            content.push(spacer(4.0));
            let headers = vec![
                "Onderdeel".to_string(),
                "Materiaal".to_string(),
                "Sterkteklasse".to_string(),
            ];
            let rows: Vec<Vec<String>> = materialen
                .iter()
                .map(|m| {
                    vec![
                        get_str(m, "onderdeel"),
                        get_str(m, "materiaal"),
                        get_str(m, "sterkteklasse"),
                    ]
                })
                .collect();
            content.push(table("Toegepaste materialen", &headers, &rows));
        }

    vec![Section {
        title: "Uitgangspunten".to_string(),
        level: 1,
        content,
        orientation: None,
        page_break_before: false,
    }]
}

fn build_belastingen(data: &Value) -> Vec<Section> {
    let bel = match data.get("belastingen") {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };

    let mut sections = Vec::new();

    // Main section
    let mut main_content = Vec::new();
    if let Some(beschr) = bel.get("beschrijving").and_then(|v| v.as_str())
        && !beschr.is_empty() {
            main_content.push(paragraph(beschr));
        }
    sections.push(Section {
        title: "Belastingen".to_string(),
        level: 1,
        content: main_content,
        orientation: None,
        page_break_before: false,
    });

    // Sub-sections per category
    if let Some(cats) = bel.get("categorieen").and_then(|v| v.as_array()) {
        for cat in cats {
            let mut cat_content = Vec::new();
            if let Some(beschr) = cat.get("beschrijving").and_then(|v| v.as_str())
                && !beschr.is_empty() {
                    cat_content.push(paragraph(beschr));
                }

            if let Some(items) = cat.get("items").and_then(|v| v.as_array())
                && !items.is_empty()
                    && let Some(first) = items.first().and_then(|i| i.as_object()) {
                        let headers: Vec<String> = first.keys().cloned().collect();
                        let rows: Vec<Vec<String>> = items
                            .iter()
                            .filter_map(|item| item.as_object())
                            .map(|obj| obj.values().map(value_to_string).collect())
                            .collect();
                        cat_content.push(table("", &headers, &rows));
                    }

            let level = cat
                .get("level")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as u8;
            sections.push(Section {
                title: get_str(cat, "titel"),
                level,
                content: cat_content,
                orientation: None,
                page_break_before: false,
            });
        }
    }

    sections
}

fn build_elementen(data: &Value) -> Vec<Section> {
    let elementen = match data.get("elementen").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut sections = Vec::new();

    for element in elementen {
        let mut content: Vec<ContentBlock> = Vec::new();

        if let Some(beschr) = element.get("beschrijving").and_then(|v| v.as_str())
            && !beschr.is_empty() {
                content.push(paragraph(beschr));
            }

        // Calculations
        if let Some(calcs) = element.get("berekeningen").and_then(|v| v.as_array()) {
            for calc in calcs {
                content.push(spacer(3.0));
                content.push(ContentBlock::Calculation(CalcSchema {
                    title: get_str(calc, "title"),
                    formula: Some(get_str(calc, "formula")),
                    substitution: opt_str(calc, "substitution"),
                    result: opt_str(calc, "result"),
                    unit: opt_str(calc, "unit"),
                    reference: opt_str(calc, "reference"),
                }));
            }
        }

        // Checks
        if let Some(checks) = element.get("checks").and_then(|v| v.as_array()) {
            for check in checks {
                content.push(spacer(3.0));
                let uc = check.get("unity_check").and_then(|v| v.as_f64());
                let limit = check
                    .get("limit")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                let result = match uc {
                    Some(val) if val <= limit => Some(CheckResult::Pass),
                    Some(_) => Some(CheckResult::Fail),
                    None => None,
                };
                content.push(ContentBlock::Check(CheckSchema {
                    description: get_str(check, "description"),
                    required_value: Some(get_str(check, "required_value")),
                    calculated_value: Some(get_str(check, "calculated_value")),
                    result,
                    unity_check: uc,
                    limit,
                    reference: opt_str(check, "reference"),
                }));
            }
        }

        sections.push(Section {
            title: get_str(element, "naam"),
            level: 1,
            content,
            orientation: None,
            page_break_before: true,
        });
    }

    sections
}

fn build_uc_overzicht(data: &Value) -> Vec<Section> {
    let elementen = match data.get("elementen").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return Vec::new(),
    };

    let headers = vec![
        "Element".to_string(),
        "Toets".to_string(),
        "UC".to_string(),
        "Resultaat".to_string(),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for element in elementen {
        let naam = get_str(element, "naam");
        if let Some(checks) = element.get("checks").and_then(|v| v.as_array()) {
            for check in checks {
                let uc = check.get("unity_check").and_then(|v| v.as_f64());
                let limit = check
                    .get("limit")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                let uc_str = uc.map(|v| format!("{v:.2}")).unwrap_or("-".to_string());
                let passes = uc.is_some_and(|v| v <= limit);
                rows.push(vec![
                    naam.clone(),
                    get_str(check, "description"),
                    uc_str,
                    if passes {
                        "VOLDOET".to_string()
                    } else {
                        "VOLDOET NIET".to_string()
                    },
                ]);
            }
        }
    }

    if rows.is_empty() {
        return Vec::new();
    }

    vec![Section {
        title: "Unity check overzicht".to_string(),
        level: 1,
        content: vec![table("Unity check overzicht", &headers, &rows)],
        orientation: None,
        page_break_before: false,
    }]
}

fn build_conclusie(data: &Value) -> Vec<Section> {
    let conclusie = match data.get("conclusie").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return Vec::new(),
    };

    vec![Section {
        title: "Conclusie".to_string(),
        level: 1,
        content: vec![paragraph(conclusie)],
        orientation: None,
        page_break_before: false,
    }]
}

// ── Daylight Report ────────────────────────────────────────────────────

/// Daglichtberekening rapport (Bouwbesluit Art. 3.74 / NEN 2057).
pub struct DaylightReport;

impl ReportBuilder for DaylightReport {
    fn report_type(&self) -> &str {
        "daylight"
    }

    fn default_template(&self) -> &str {
        "daylight_report"
    }

    fn build_sections(&self, data: &Value) -> Vec<Section> {
        let mut sections = Vec::new();
        sections.extend(daylight_uitgangspunten(data));
        sections.extend(daylight_situatie(data));
        sections.extend(daylight_ruimtes(data));
        sections.extend(daylight_toetsing(data));
        sections.extend(build_conclusie(data));
        sections
    }
}

fn daylight_uitgangspunten(data: &Value) -> Vec<Section> {
    let uitg = match data.get("uitgangspunten") {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };

    let mut content: Vec<ContentBlock> = Vec::new();

    if let Some(beschr) = uitg.get("beschrijving").and_then(|v| v.as_str())
        && !beschr.is_empty() {
            content.push(paragraph(beschr));
        }

    let mut details = Vec::new();
    if let Some(v) = uitg.get("norm").and_then(|v| v.as_str())
        && !v.is_empty() {
            details.push(format!("Norm: {v}"));
        }
    if let Some(v) = uitg.get("gebouwtype").and_then(|v| v.as_str())
        && !v.is_empty() {
            details.push(format!("Gebouwtype: {v}"));
        }
    if let Some(v) = uitg.get("gebruiksfunctie").and_then(|v| v.as_str())
        && !v.is_empty() {
            details.push(format!("Gebruiksfunctie: {v}"));
        }
    if !details.is_empty() {
        content.push(paragraph(&details.join("\n")));
    }

    vec![Section {
        title: "Uitgangspunten".to_string(),
        level: 1,
        content,
        orientation: None,
        page_break_before: false,
    }]
}

fn daylight_situatie(data: &Value) -> Vec<Section> {
    let sit = match data.get("situatie") {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };

    let mut content: Vec<ContentBlock> = Vec::new();

    if let Some(beschr) = sit.get("beschrijving").and_then(|v| v.as_str())
        && !beschr.is_empty() {
            content.push(paragraph(beschr));
        }
    if let Some(ori) = sit.get("orientatie").and_then(|v| v.as_str())
        && !ori.is_empty() {
            content.push(paragraph(&format!("Oriëntatie: {ori}")));
        }

    vec![Section {
        title: "Situatie en oriëntatie".to_string(),
        level: 1,
        content,
        orientation: None,
        page_break_before: false,
    }]
}

fn daylight_ruimtes(data: &Value) -> Vec<Section> {
    let ruimtes = match data.get("ruimtes").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return Vec::new(),
    };

    let mut sections = Vec::new();

    // Main section header
    sections.push(Section {
        title: "Daglichtberekening per ruimte".to_string(),
        level: 1,
        content: vec![paragraph(
            "Per verblijfsgebied wordt de equivalente daglichtoppervlakte \
             berekend conform NEN 2057.",
        )],
        orientation: None,
        page_break_before: false,
    });

    for ruimte in ruimtes {
        let naam = get_str(ruimte, "naam");
        let vloer = ruimte
            .get("vloeroppervlakte_m2")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let eis_pct = ruimte
            .get("eis_percentage")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0);

        let mut content: Vec<ContentBlock> = Vec::new();

        let mut info = format!("Vloeroppervlakte: {vloer:.1} m²");
        if let Some(verd) = ruimte.get("verdieping").and_then(|v| v.as_str())
            && !verd.is_empty() {
                info.push_str(&format!(" | Verdieping: {verd}"));
            }
        content.push(paragraph(&info));

        // Windows table
        let ramen = ruimte.get("ramen").and_then(|v| v.as_array());
        if let Some(ramen) = ramen
            && !ramen.is_empty() {
                content.push(spacer(3.0));
                let headers = vec![
                    "Raam".to_string(),
                    "B×H (m)".to_string(),
                    "Oriëntatie".to_string(),
                    "A_glas (m²)".to_string(),
                    "A_eq (m²)".to_string(),
                ];
                let mut rows: Vec<Vec<String>> = Vec::new();
                let mut totaal_eq = 0.0;

                for raam in ramen {
                    let b = raam
                        .get("breedte_m")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let h = raam
                        .get("hoogte_m")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let a_glas = b * h;

                    let red = raam.get("reductiefactoren");
                    let f_beb = red
                        .and_then(|r| r.get("bebouwing"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0);
                    let f_vuil = red
                        .and_then(|r| r.get("vuil"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0);
                    let f_con = red
                        .and_then(|r| r.get("constructie"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0);
                    let a_eq = a_glas * f_beb * f_vuil * f_con;
                    totaal_eq += a_eq;

                    rows.push(vec![
                        get_str(raam, "naam"),
                        format!("{b:.2} × {h:.2}"),
                        get_str(raam, "orientatie"),
                        format!("{a_glas:.2}"),
                        format!("{a_eq:.2}"),
                    ]);
                }

                content.push(table("Raamopeningen", &headers, &rows));

                // Calculation
                content.push(spacer(3.0));
                let eis_abs = vloer * eis_pct / 100.0;
                content.push(ContentBlock::Calculation(CalcSchema {
                    title: format!("Equivalente daglichtoppervlakte — {naam}"),
                    formula: Some(
                        "A_eq,totaal = Σ (A_glas × f_bebouwing × f_vuil × f_constructie)"
                            .to_string(),
                    ),
                    substitution: None,
                    result: Some(format!("{totaal_eq:.2}")),
                    unit: Some("m²".to_string()),
                    reference: Some("NEN 2057".to_string()),
                }));

                // Check
                content.push(spacer(3.0));
                let uc = if eis_abs > 0.0 {
                    totaal_eq / eis_abs
                } else {
                    0.0
                };
                let inv_uc = if uc > 0.0 { 1.0 / uc } else { 999.0 };
                let result = if inv_uc <= 1.0 {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail
                };
                content.push(ContentBlock::Check(CheckSchema {
                    description: format!("Daglichttoetreding {naam}"),
                    required_value: Some(format!(
                        "A_eq ≥ {eis_pct}% × {vloer:.1} = {eis_abs:.2} m²"
                    )),
                    calculated_value: Some(format!("A_eq = {totaal_eq:.2} m²")),
                    result: Some(result),
                    unity_check: Some(inv_uc),
                    limit: 1.0,
                    reference: Some("Bouwbesluit art. 3.74".to_string()),
                }));
            }

        sections.push(Section {
            title: naam,
            level: 2,
            content,
            orientation: None,
            page_break_before: false,
        });
    }

    sections
}

fn daylight_toetsing(data: &Value) -> Vec<Section> {
    let ruimtes = match data.get("ruimtes").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return Vec::new(),
    };

    let headers = vec![
        "Ruimte".to_string(),
        "A_vloer (m²)".to_string(),
        "Eis (m²)".to_string(),
        "A_eq (m²)".to_string(),
        "Resultaat".to_string(),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for ruimte in ruimtes {
        let naam = get_str(ruimte, "naam");
        let vloer = ruimte
            .get("vloeroppervlakte_m2")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let eis_pct = ruimte
            .get("eis_percentage")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0);
        let eis_abs = vloer * eis_pct / 100.0;

        let mut totaal_eq = 0.0;
        if let Some(ramen) = ruimte.get("ramen").and_then(|v| v.as_array()) {
            for raam in ramen {
                let b = raam
                    .get("breedte_m")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let h = raam
                    .get("hoogte_m")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let red = raam.get("reductiefactoren");
                let factor = red
                    .and_then(|r| r.get("bebouwing"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0)
                    * red
                        .and_then(|r| r.get("vuil"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0)
                    * red
                        .and_then(|r| r.get("constructie"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0);
                totaal_eq += b * h * factor;
            }
        }

        let voldoet = totaal_eq >= eis_abs;
        rows.push(vec![
            naam,
            format!("{vloer:.1}"),
            format!("{eis_abs:.2}"),
            format!("{totaal_eq:.2}"),
            if voldoet {
                "VOLDOET".to_string()
            } else {
                "VOLDOET NIET".to_string()
            },
        ]);
    }

    vec![Section {
        title: "Toetsingsoverzicht".to_string(),
        level: 1,
        content: vec![table(
            "Samenvattend overzicht daglichttoetreding",
            &headers,
            &rows,
        )],
        orientation: None,
        page_break_before: false,
    }]
}

// ── Building Code Report ───────────────────────────────────────────────

/// Bouwbesluit toetsing rapport.
pub struct BuildingCodeReport;

impl ReportBuilder for BuildingCodeReport {
    fn report_type(&self) -> &str {
        "building_code"
    }

    fn default_template(&self) -> &str {
        "building_code_report"
    }

    fn build_sections(&self, data: &Value) -> Vec<Section> {
        let mut sections = Vec::new();
        sections.extend(bc_uitgangspunten(data));
        sections.extend(bc_hoofdstukken(data));
        sections.extend(bc_overzicht(data));
        sections.extend(build_conclusie(data));
        sections
    }
}

fn bc_uitgangspunten(data: &Value) -> Vec<Section> {
    let uitg = match data.get("uitgangspunten") {
        Some(v) if v.is_object() => v,
        _ => return Vec::new(),
    };

    let mut content: Vec<ContentBlock> = Vec::new();

    if let Some(beschr) = uitg.get("beschrijving").and_then(|v| v.as_str())
        && !beschr.is_empty() {
            content.push(paragraph(beschr));
        }

    let mut details = Vec::new();
    if let Some(v) = uitg.get("gebruiksfunctie").and_then(|v| v.as_str())
        && !v.is_empty() {
            details.push(format!("Gebruiksfunctie: {v}"));
        }
    if let Some(v) = uitg.get("bouwbesluit_versie").and_then(|v| v.as_str())
        && !v.is_empty() {
            details.push(format!("Bouwbesluit: {v}"));
        }
    if let Some(nieuwbouw) = uitg.get("nieuwbouw").and_then(|v| v.as_bool()) {
        details.push(format!(
            "Toetsingsniveau: {}",
            if nieuwbouw {
                "Nieuwbouw"
            } else {
                "Bestaande bouw"
            }
        ));
    }
    if !details.is_empty() {
        content.push(paragraph(&details.join("\n")));
    }

    vec![Section {
        title: "Projectgegevens en uitgangspunten".to_string(),
        level: 1,
        content,
        orientation: None,
        page_break_before: false,
    }]
}

fn bc_hoofdstukken(data: &Value) -> Vec<Section> {
    let hoofdstukken = match data.get("hoofdstukken").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut sections = Vec::new();

    for hoofdstuk in hoofdstukken {
        let titel = get_str(hoofdstuk, "titel");
        let mut h_content: Vec<ContentBlock> = Vec::new();

        if let Some(beschr) = hoofdstuk.get("beschrijving").and_then(|v| v.as_str())
            && !beschr.is_empty() {
                h_content.push(paragraph(beschr));
            }

        if let Some(toetsingen) = hoofdstuk.get("toetsingen").and_then(|v| v.as_array()) {
            for toets in toetsingen {
                h_content.push(spacer(3.0));

                let artikel = get_str(toets, "artikel");
                let beschr = get_str(toets, "beschrijving");
                let label = if !artikel.is_empty() && !beschr.is_empty() {
                    format!("{artikel} — {beschr}")
                } else if !artikel.is_empty() {
                    artikel.clone()
                } else {
                    beschr
                };

                let voldoet = toets
                    .get("voldoet")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let result = if voldoet {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail
                };

                h_content.push(ContentBlock::Check(CheckSchema {
                    description: label,
                    required_value: Some(get_str(toets, "eis")),
                    calculated_value: Some(get_str(toets, "berekend")),
                    result: Some(result),
                    unity_check: None,
                    limit: 1.0,
                    reference: if artikel.is_empty() {
                        None
                    } else {
                        Some(artikel)
                    },
                }));

                if let Some(toelichting) = toets.get("toelichting").and_then(|v| v.as_str())
                    && !toelichting.is_empty() {
                        h_content.push(paragraph(toelichting));
                    }
            }
        }

        sections.push(Section {
            title: titel,
            level: 1,
            content: h_content,
            orientation: None,
            page_break_before: false,
        });
    }

    sections
}

fn bc_overzicht(data: &Value) -> Vec<Section> {
    let hoofdstukken = match data.get("hoofdstukken").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return Vec::new(),
    };

    let headers = vec![
        "Artikel".to_string(),
        "Omschrijving".to_string(),
        "Eis".to_string(),
        "Berekend".to_string(),
        "Resultaat".to_string(),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for hoofdstuk in hoofdstukken {
        if let Some(toetsingen) = hoofdstuk.get("toetsingen").and_then(|v| v.as_array()) {
            for toets in toetsingen {
                let voldoet = toets
                    .get("voldoet")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                rows.push(vec![
                    get_str(toets, "artikel"),
                    get_str(toets, "beschrijving"),
                    get_str(toets, "eis"),
                    get_str(toets, "berekend"),
                    if voldoet {
                        "VOLDOET".to_string()
                    } else {
                        "VOLDOET NIET".to_string()
                    },
                ]);
            }
        }
    }

    if rows.is_empty() {
        return Vec::new();
    }

    vec![Section {
        title: "Samenvattend overzicht".to_string(),
        level: 1,
        content: vec![table("Samenvattend toetsingsoverzicht", &headers, &rows)],
        orientation: None,
        page_break_before: false,
    }]
}

// ── Helper functions ───────────────────────────────────────────────────

fn paragraph(text: &str) -> ContentBlock {
    ContentBlock::Paragraph(ParagraphBlock {
        text: text.to_string(),
        style: "Normal".to_string(),
    })
}

fn spacer(height_mm: f64) -> ContentBlock {
    ContentBlock::Spacer(crate::schema::SpacerBlock { height_mm })
}

fn table(title: &str, headers: &[String], rows: &[Vec<String>]) -> ContentBlock {
    ContentBlock::Table(TableSchema {
        title: if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        },
        headers: headers.to_vec(),
        rows: rows.iter().map(|r| r.iter().map(|c| Value::String(c.clone())).collect()).collect(),
        column_widths: None,
        style: Default::default(),
    })
}

fn get_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn opt_str(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_builder_structural() {
        let builder = get_builder("structural").unwrap();
        assert_eq!(builder.report_type(), "structural");
        assert_eq!(builder.default_template(), "structural_report");
    }

    #[test]
    fn test_get_builder_daylight() {
        let builder = get_builder("daylight").unwrap();
        assert_eq!(builder.report_type(), "daylight");
    }

    #[test]
    fn test_get_builder_building_code() {
        let builder = get_builder("building_code").unwrap();
        assert_eq!(builder.report_type(), "building_code");
    }

    #[test]
    fn test_get_builder_unknown() {
        assert!(get_builder("unknown").is_none());
    }

    #[test]
    fn test_structural_empty_data() {
        let builder = StructuralReport;
        let sections = builder.build_sections(&json!({}));
        assert!(sections.is_empty());
    }

    #[test]
    fn test_structural_uitgangspunten() {
        let data = json!({
            "uitgangspunten": {
                "beschrijving": "Test beschrijving",
                "normen": ["NEN-EN 1990", "NEN-EN 1991-1"],
                "materialen": [
                    {"onderdeel": "Kolom", "materiaal": "Staal", "sterkteklasse": "S235"}
                ]
            }
        });
        let builder = StructuralReport;
        let sections = builder.build_sections(&data);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Uitgangspunten");
        // beschrijving + normen + spacer + table
        assert!(sections[0].content.len() >= 3);
    }

    #[test]
    fn test_structural_elements_with_checks() {
        let data = json!({
            "elementen": [{
                "naam": "Kolom K1",
                "beschrijving": "HEB 200",
                "berekeningen": [{
                    "title": "Axiale druk",
                    "formula": "N_Ed / N_Rd",
                    "result": "0.75",
                    "unit": "-"
                }],
                "checks": [{
                    "description": "Druktoets",
                    "required_value": "N_Rd = 1000 kN",
                    "calculated_value": "N_Ed = 750 kN",
                    "unity_check": 0.75,
                    "limit": 1.0
                }]
            }]
        });
        let builder = StructuralReport;
        let sections = builder.build_sections(&data);
        // Element section + UC overzicht
        assert!(sections.len() >= 2);
        assert_eq!(sections[0].title, "Kolom K1");
        assert!(sections[0].page_break_before);
    }

    #[test]
    fn test_daylight_ruimtes() {
        let data = json!({
            "ruimtes": [{
                "naam": "Woonkamer",
                "vloeroppervlakte_m2": 25.0,
                "eis_percentage": 10.0,
                "ramen": [{
                    "naam": "R1",
                    "breedte_m": 1.5,
                    "hoogte_m": 2.0,
                    "orientatie": "Zuid",
                    "reductiefactoren": {"bebouwing": 0.9, "vuil": 0.95, "constructie": 0.8}
                }]
            }]
        });
        let builder = DaylightReport;
        let sections = builder.build_sections(&data);
        // Header section + room section + toetsing
        assert!(sections.len() >= 3);
    }

    #[test]
    fn test_building_code_hoofdstukken() {
        let data = json!({
            "hoofdstukken": [{
                "titel": "Brandveiligheid",
                "beschrijving": "Toetsing aan afdeling 2.1",
                "toetsingen": [{
                    "artikel": "Art. 2.67",
                    "beschrijving": "Weerstand tegen branddoorslag",
                    "eis": "60 min",
                    "berekend": "90 min",
                    "voldoet": true
                }]
            }]
        });
        let builder = BuildingCodeReport;
        let sections = builder.build_sections(&data);
        assert!(!sections.is_empty());
        assert_eq!(sections[0].title, "Brandveiligheid");
    }

    #[test]
    fn test_conclusie() {
        let data = json!({"conclusie": "Het project voldoet."});
        let sections = build_conclusie(&data);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Conclusie");
    }

    #[test]
    fn test_conclusie_empty() {
        let sections = build_conclusie(&json!({}));
        assert!(sections.is_empty());
    }
}
