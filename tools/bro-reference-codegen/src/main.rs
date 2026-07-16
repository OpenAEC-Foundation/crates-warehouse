use std::{collections::BTreeMap, env, error::Error, fs, path::PathBuf};

use serde::Deserialize;

const ENDPOINT: &str = "https://publiek.broservices.nl/bro/refcodes/v1/codes";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct CodeEntry {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainVersion {
    ref_codes: Vec<CodeEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainResponse {
    ref_domain_versions: Vec<DomainVersion>,
}

struct CodeSet {
    variant: &'static str,
    constant: &'static str,
    domains: &'static [&'static str],
    seeds: &'static [(&'static str, &'static str)],
}

const CODE_SETS: &[CodeSet] = &[
    CodeSet {
        variant: "GeotechnicalSoilName",
        constant: "GEOTECHNICAL_SOIL_NAMES",
        domains: &["urn:bro:bhrgt:GeotechnicalSoilName"],
        seeds: &[
            ("matigFijnZand", "Matig fijn zand"),
            ("sterkSiltigeKlei", "Sterk siltige klei"),
        ],
    },
    CodeSet {
        variant: "Lithology",
        constant: "LITHOLOGIES",
        domains: &["urn:bro:bhrg:GeologicalSoilName"],
        seeds: &[("klei", "Klei"), ("zand", "Zand")],
    },
    CodeSet {
        variant: "Colour",
        constant: "COLOURS",
        domains: &["urn:bro:bhrgt:Colour", "urn:bro:bhrg:Colour"],
        seeds: &[("bruin", "Bruin"), ("geel", "Geel")],
    },
    CodeSet {
        variant: "QualityRegime",
        constant: "QUALITY_REGIMES",
        domains: &[],
        seeds: &[
            ("IMBRO", "IMBRO-kwaliteitsregime"),
            ("IMBRO/A", "IMBRO/A-kwaliteitsregime"),
        ],
    },
];

fn main() -> Result<(), Box<dyn Error>> {
    let output = output_path()?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("bro-reference-codegen")
        .build()?;
    let mut tables = Vec::new();

    for set in CODE_SETS {
        let domain_payloads = set
            .domains
            .iter()
            .map(|domain| fetch_codes(&client, domain))
            .collect::<Result<Vec<_>, _>>()?;
        let entries = merge_code_payloads(domain_payloads, set.seeds);
        tables.push((set, entries));
    }

    fs::write(output, render_module(&tables))?;
    Ok(())
}

fn merge_code_payloads(
    domain_payloads: Vec<Vec<CodeEntry>>,
    seeds: &[(&str, &str)],
) -> Vec<CodeEntry> {
    domain_payloads
        .into_iter()
        .flatten()
        .chain(seeds.iter().map(|(code, description)| CodeEntry {
            code: (*code).to_owned(),
            description: (*description).to_owned(),
        }))
        .collect()
}

fn output_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let output = arguments
        .next()
        .ok_or("usage: bro-reference-codegen <output-path>")?;
    if arguments.next().is_some() {
        return Err("usage: bro-reference-codegen <output-path>".into());
    }
    Ok(output.into())
}

fn fetch_codes(
    client: &reqwest::blocking::Client,
    domain: &str,
) -> Result<Vec<CodeEntry>, reqwest::Error> {
    let response = client
        .get(ENDPOINT)
        .query(&[("version", "latest"), ("domain", domain)])
        .send()?
        .error_for_status()?
        .json::<DomainResponse>()?;
    Ok(response
        .ref_domain_versions
        .into_iter()
        .flat_map(|version| version.ref_codes)
        .collect())
}

fn render_table(name: &str, entries: Vec<CodeEntry>) -> String {
    let entries: BTreeMap<_, _> = entries
        .into_iter()
        .map(|entry| (entry.code, entry.description))
        .collect();
    let mut output = format!("const {name}: &[(&str, &str)] = &[\n");
    for (code, description) in entries {
        output.push_str(&format!("    ({code:?}, {description:?}),\n"));
    }
    output.push_str("];\n");
    output
}

fn render_module(tables: &[(&CodeSet, Vec<CodeEntry>)]) -> String {
    let mut output = String::from(
        "//! Human-readable descriptions for selected BRO reference-code sets.\n\n\
         /// A supported BRO reference-code set.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub enum ReferenceCodeSet {\n",
    );
    for (set, _) in tables {
        output.push_str(&format!("    /// Reference codes for {}.\n", set.variant));
        output.push_str(&format!("    {},\n", set.variant));
    }
    output.push_str("}\n\n");
    for (set, entries) in tables {
        output.push_str(&render_table(set.constant, entries.clone()));
        output.push('\n');
    }
    output.push_str(
        "/// Returns the Dutch description of a known reference code.\n\
         ///\n\
         /// Unknown values return `None`; parsed documents retain their original code\n\
         /// strings independently of this optional lookup.\n\
         pub fn describe_reference_code(set: ReferenceCodeSet, code: &str) -> Option<&'static str> {\n\
         \x20   let entries = match set {\n",
    );
    for (set, _) in tables {
        output.push_str(&format!(
            "        ReferenceCodeSet::{} => {},\n",
            set.variant, set.constant
        ));
    }
    output.push_str(
        "    };\n\
         \x20   entries.binary_search_by_key(&code, |(candidate, _)| *candidate)\n\
         \x20       .ok().map(|index| entries[index].1)\n\
         }\n",
    );
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tables_are_sorted_and_rust_strings_are_escaped() {
        let rendered = render_table(
            "CODES",
            vec![
                CodeEntry {
                    code: "zand".to_owned(),
                    description: "regel\n\"twee\"".to_owned(),
                },
                CodeEntry {
                    code: "klei".to_owned(),
                    description: "back\\slash".to_owned(),
                },
            ],
        );

        assert!(rendered.find("klei").unwrap() < rendered.find("zand").unwrap());
        assert!(rendered.contains(r#""regel\n\"twee\"""#));
        assert!(rendered.contains(r#""back\\slash""#));
    }

    #[test]
    fn complete_module_documents_every_public_variant() {
        let tables: Vec<_> = CODE_SETS
            .iter()
            .map(|set| (set, Vec::<CodeEntry>::new()))
            .collect();
        let rendered = render_module(&tables);

        for set in CODE_SETS {
            assert!(
                rendered.contains(&format!("    /// Reference codes for {}.\n", set.variant)),
                "missing rustdoc for {}",
                set.variant
            );
        }
    }

    #[test]
    fn colour_combines_domain_payloads_into_one_sorted_table() {
        let colour = CODE_SETS
            .iter()
            .find(|set| set.variant == "Colour")
            .unwrap();
        assert_eq!(
            colour.domains,
            &["urn:bro:bhrgt:Colour", "urn:bro:bhrg:Colour",]
        );

        let entries = merge_code_payloads(
            vec![
                vec![CodeEntry {
                    code: "paars".to_owned(),
                    description: "BHR-GT paars".to_owned(),
                }],
                vec![CodeEntry {
                    code: "roze".to_owned(),
                    description: "BHR-G roze".to_owned(),
                }],
            ],
            &[],
        );
        let rendered = render_table(colour.constant, entries);

        assert!(rendered.find("paars").unwrap() < rendered.find("roze").unwrap());
        assert!(rendered.contains(r#"("paars", "BHR-GT paars")"#));
        assert!(rendered.contains(r#"("roze", "BHR-G roze")"#));
    }

    #[test]
    fn later_domain_payload_wins_duplicate_codes_deterministically() {
        let entries = merge_code_payloads(
            vec![
                vec![CodeEntry {
                    code: "bruin".to_owned(),
                    description: "eerste domein".to_owned(),
                }],
                vec![CodeEntry {
                    code: "bruin".to_owned(),
                    description: "tweede domein".to_owned(),
                }],
            ],
            &[],
        );
        let rendered = render_table("COLOURS", entries);

        assert_eq!(rendered.matches(r#"("bruin", "#).count(), 1);
        assert!(!rendered.contains("eerste domein"));
        assert!(rendered.contains(r#"("bruin", "tweede domein")"#));
    }
}
