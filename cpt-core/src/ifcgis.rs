//! `.ifcgis` — Open GEO Studio project file format.
//!
//! A JSON container in IFCX-flavoured style that bundles project metadata
//! plus all CPTs of a project into a single file. Follows the OpenAEC
//! ecosystem's preference for IFCX (IFC5 alpha) — flat object lists, stable
//! types, mergeable in git.
//!
//! For v1 we don't depend on the full IFCX schema (still in flux); we use
//! OpenGEO-prefixed types (`OpenGeoProject`, `OpenGeoCpt`) and the IFCX
//! conventions for header + object listing. A future `cpt-ifcx` crate can
//! map this into strict IFCX once that schema stabilises.
//!
//! File extension: `.ifcgis`
//! Media type: `application/x.ifcgis+json`

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::Cpt;
use crate::error::CptError;

/// Borings are still parsed entirely on the TypeScript side (BHR-XML
/// DOM walker in `apps/desktop/src/types/bore.ts`), so cpt-core has no
/// strict Bore struct. Storing them as opaque JSON values keeps the
/// ifcgis schema author-agnostic and forward-compatible — the frontend
/// validates the actual shape on load.
pub type BoreJson = serde_json::Value;

// Schema-versie. Sprong naar 0.2 omdat het bestandsmodel niet
// backwards-compatible is met de oorspronkelijke ifcgis-0.1: er zijn
// nieuwe top-level secties (bores, tekening, crs) bijgekomen die de
// 0.1-loader simpelweg negeerde via `#[serde(default)]`.
const SCHEMA_VERSION: &str = "ifcgis-0.2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub header: Header,
    pub project: ProjectInfo,
    #[serde(default)]
    pub cpts: Vec<Cpt>,
    /// Borings — sinds 0.2. Voor oudere bestanden ontbreekt het veld;
    /// `#[serde(default)]` zorgt voor een lege Vec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bores: Vec<BoreJson>,
    /// Coordinate Reference System voor de RD-coördinaten in `cpts`
    /// en `bores`. Standaard `EPSG:28992` (Amersfoort / RD New).
    #[serde(default)]
    pub crs: Crs,
    /// Tekening-state — paper, schaal, marker-placeringen, raster,
    /// lijnen, RD-tags, overlay. Optioneel zodat een project zonder
    /// tekening (alleen ruwe CPTs) ook valide blijft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tekening: Option<TekeningLayout>,
    /// Title-block velden (Projectnaam, Projectnr, Adres,
    /// Tekening-nr, Schaal-label, Datum, Getekend, Gecontroleerd,
    /// Versie). Wordt door de Situatietekening-tab gevuld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_block: Option<TitleBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub schema: String,
    pub originating_system: String,
    pub timestamp: String, // RFC3339
}

impl Header {
    pub fn new(originating_system: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA_VERSION.into(),
            originating_system: originating_system.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(rename = "type", default = "default_project_type")]
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub client: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub location: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub project_number: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub author: String,
    pub date: NaiveDate,
}

fn default_project_type() -> String {
    "OpenGeoProject".to_string()
}

/// Coordinate Reference System metadata. Default is Amersfoort / RD
/// New (EPSG:28992) — de standaard voor NL-geotechniek-projecten.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crs {
    pub epsg: u32,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub name: String,
}

impl Default for Crs {
    fn default() -> Self {
        Self {
            epsg: 28992,
            name: "Amersfoort / RD New".to_string(),
        }
    }
}

/// Volledige tekening-state — alles wat nodig is om een Situatie-
/// tekening exact te herstellen. Coordinaten in lat/lon (EPSG:4326)
/// voor compat met Leaflet; conversie naar RD via WGS84_TO_RD in de
/// frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TekeningLayout {
    /// "A2" of "A3" (altijd landscape).
    pub paper_size: String,
    /// Print-schaal — 1:N, opgeslagen als N (b.v. 500, 1000, 2000).
    pub scale: u32,
    /// Centrum van de map-viewport in WGS84 lat/lon + zoom-niveau.
    pub center: ViewCenter,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<PlacedMarker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rasters: Vec<RasterLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<DrawnLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coord_tags: Vec<CoordTag>,
    /// Image / raster overlay (geprojecteerd op de kaart). `src` is
    /// een data-URL of een relatief pad — de frontend bepaalt welke
    /// vorm gebruikt wordt bij opslaan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<OverlayInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewCenter {
    pub lat: f64,
    pub lon: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedMarker {
    pub id: String,
    /// "sondering" of "bore" — bepaalt het kaart-symbool.
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    /// Sleeve-friction (kleefmeting) flag — alleen relevant voor
    /// sondering-markers.
    #[serde(default, skip_serializing_if = "is_false")]
    pub kleefmeting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterLayout {
    pub id: String,
    pub center_lat: f64,
    pub center_lon: f64,
    pub rows: u32,
    pub cols: u32,
    /// H-o-h afstand in meters, X- en Y-as.
    pub spacing_x: f64,
    pub spacing_y: f64,
    /// Rotatie in graden klokwise vanaf noord.
    pub rotation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawnLine {
    pub id: String,
    /// "line" of "dimension".
    pub kind: String,
    pub lat1: f64,
    pub lon1: f64,
    pub lat2: f64,
    pub lon2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordTag {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayInfo {
    pub id: String,
    pub name: String,
    /// "image" / "svg" / "pdf" / "dwg" — sturend voor de renderer.
    pub kind: String,
    /// data-URL (base64 PNG/SVG/PDF) zodat het bestand zelfcontained
    /// is. Voor grote afbeeldingen kan dit MB's beslaan; dat is
    /// bewust — een ifcgis is bedoeld als single-file project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// Breedte in meters (real-world). Hoogte wordt afgeleid uit de
    /// aspect-ratio van de image bij rendering.
    pub width_meters: f64,
    /// Plaatsing — center coördinaten van de overlay op de kaart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_lon: Option<f64>,
}

/// Title-block velden — wordt in de tekening rechtsonder als
/// metadata-band geprint. Alle velden zijn optioneel zodat een
/// minimaal project ook geldig is.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TitleBlock {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub address: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub drawing_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scale: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub drawn_by: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub checked_by: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Serialise a project (metadata + CPTs + optional bores/tekening/
/// title_block) to a pretty-printed `.ifcgis` JSON string.
pub fn save(project: ProjectInfo, cpts: Vec<Cpt>) -> Result<String, CptError> {
    save_full(project, cpts, Vec::new(), None, None)
}

/// Full save — including bores, tekening-layout, title-block.
/// `save()` blijft als shortcut voor backward compat met bestaande
/// Tauri-commands die alleen CPTs hebben.
pub fn save_full(
    project: ProjectInfo,
    cpts: Vec<Cpt>,
    bores: Vec<BoreJson>,
    tekening: Option<TekeningLayout>,
    title_block: Option<TitleBlock>,
) -> Result<String, CptError> {
    let file = ProjectFile {
        header: Header::new("Open Geotechniek Studio"),
        project,
        cpts,
        bores,
        crs: Crs::default(),
        tekening,
        title_block,
    };
    serde_json::to_string_pretty(&file)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis serialize: {e}")))
}

/// Parse a `.ifcgis` JSON file into the project model. Tolerates the
/// older `ifcgis-0.1` schema (no bores, no tekening, no crs) — those
/// fields just default to empty / None.
pub fn load(text: &str) -> Result<ProjectFile, CptError> {
    let file: ProjectFile = serde_json::from_str(text)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis parse: {e}")))?;
    if !file.header.schema.starts_with("ifcgis-") {
        return Err(CptError::InvalidGef(format!(
            "unrecognized schema '{}' (expected ifcgis-*)",
            file.header.schema
        )));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Metadata, MeasurementPoint, Position};

    fn sample_cpt(id: &str) -> Cpt {
        Cpt {
            id: id.to_string(),
            metadata: Metadata { source_file: format!("{id}.gef"), ..Default::default() },
            position: Some(Position { x_rd: 100_000.0, y_rd: 400_000.0, z_nap: Some(2.5) }),
            points: vec![
                MeasurementPoint { depth: 0.02, depth_nap: Some(2.48), qc: Some(1.5), fs: Some(0.015), rf: Some(1.0), u2: None, inclination: None },
                MeasurementPoint { depth: 0.04, depth_nap: Some(2.46), qc: Some(1.6), fs: Some(0.016), rf: Some(1.0), u2: None, inclination: None },
            ],
        }
    }

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            kind: default_project_type(),
            title: "Test project".into(),
            client: "ACME bv".into(),
            location: "Amsterdam".into(),
            project_number: "2026-001".into(),
            author: "Open GEO Studio".into(),
            date: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        }
    }

    #[test]
    fn round_trip_empty_project() {
        let json = save(sample_project(), vec![]).unwrap();
        // Schema-versie liep mee naar 0.2 toen we bores + tekening +
        // crs + title_block toevoegden — een 0.1-loader zou de extra
        // velden negeren, maar oude bestanden zonder 'bores' enz. zijn
        // wel forward-compatible omdat alle nieuwe velden #[serde(default)]
        // hebben.
        assert!(
            json.contains("\"schema\": \"ifcgis-0.2\""),
            "expected schema 0.2 in output, got: {json}"
        );
        let back = load(&json).unwrap();
        assert_eq!(back.project.title, "Test project");
        assert_eq!(back.cpts.len(), 0);
    }

    #[test]
    fn round_trip_with_cpts() {
        let json = save(sample_project(), vec![sample_cpt("S01"), sample_cpt("S02")]).unwrap();
        let back = load(&json).unwrap();
        assert_eq!(back.cpts.len(), 2);
        assert_eq!(back.cpts[0].id, "S01");
        assert_eq!(back.cpts[0].points.len(), 2);
        assert_eq!(back.cpts[0].position.unwrap().x_rd, 100_000.0);
    }

    #[test]
    fn rejects_unknown_schema() {
        let bad = r#"{"header":{"schema":"openfoo-1","originating_system":"X","timestamp":"2026-01-01T00:00:00Z"},"project":{"type":"OpenGeoProject","title":"T","date":"2026-01-01"},"cpts":[]}"#;
        let err = load(bad).err().unwrap();
        assert!(format!("{err}").contains("unrecognized schema"));
    }
}
