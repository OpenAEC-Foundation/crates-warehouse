//! Human-readable descriptions for selected BRO reference-code sets.

/// A supported BRO reference-code set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceCodeSet {
    /// Geotechnical soil names used in BHR-GT documents.
    GeotechnicalSoilName,
    /// Lithology values used in BHR-G documents.
    Lithology,
    /// Colour values used in borehole descriptions.
    Colour,
    /// BRO data-quality regimes.
    QualityRegime,
}

const GEOTECHNICAL_SOIL_NAMES: &[(&str, &str)] = &[
    ("grind", "Grind"),
    ("klei", "Klei"),
    ("matigFijnZand", "Matig fijn zand"),
    ("sterkSiltigeKlei", "Sterk siltige klei"),
    ("veen", "Veen"),
    ("zand", "Zand"),
];

const LITHOLOGIES: &[(&str, &str)] = &[
    ("grind", "Grind"),
    ("klei", "Klei"),
    ("leem", "Leem"),
    ("veen", "Veen"),
    ("zand", "Zand"),
];

const COLOURS: &[(&str, &str)] = &[
    ("beige", "Beige"),
    ("blauw", "Blauw"),
    ("bruin", "Bruin"),
    ("geel", "Geel"),
    ("grijs", "Grijs"),
    ("groen", "Groen"),
    ("oranje", "Oranje"),
    ("rood", "Rood"),
    (
        "roze",
        "Roze omvat de Munsellkleuren 10R 8/3, 10R 8/4, 2.5YR 8/3, 2.5YR 8/4, 5YR 7/3, 5YR 7/4, 5YR 8/3, 5YR 8/4, 7.5YR 7/3, 7.5YR 7/4, 7.5YR 8/3 en 7.5YR 8/4 (pink).",
    ),
    ("wit", "Wit"),
    ("zwart", "Zwart"),
];

const QUALITY_REGIMES: &[(&str, &str)] = &[
    ("IMBRO", "IMBRO-kwaliteitsregime"),
    ("IMBRO/A", "IMBRO/A-kwaliteitsregime"),
];

/// Returns the Dutch description of a known reference code.
///
/// Unknown values return `None`; parsed documents retain their original code
/// strings independently of this optional lookup.
pub fn describe_reference_code(set: ReferenceCodeSet, code: &str) -> Option<&'static str> {
    let entries = match set {
        ReferenceCodeSet::GeotechnicalSoilName => GEOTECHNICAL_SOIL_NAMES,
        ReferenceCodeSet::Lithology => LITHOLOGIES,
        ReferenceCodeSet::Colour => COLOURS,
        ReferenceCodeSet::QualityRegime => QUALITY_REGIMES,
    };
    entries
        .binary_search_by_key(&code, |(candidate, _)| *candidate)
        .ok()
        .map(|index| entries[index].1)
}
