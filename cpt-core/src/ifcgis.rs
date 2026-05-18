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

// Schema-versie. Sprong naar 0.3 omdat er nieuwe top-level secties
// bijkomen (`gis` met layer-lijst, `deliverable` met IFC-stijl 2D
// representatie). Oudere 0.2/0.1 bestanden blijven laden — alle
// nieuwe velden zijn `#[serde(default, skip_serializing_if = ...)]`
// zodat ze niet hoeven te bestaan in input én niet uitgeschreven
// worden wanneer ze leeg/None zijn (forward + backward compat).
const SCHEMA_VERSION: &str = "ifcgis-0.3";

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
    /// GIS-metadata (sinds 0.3) — vervangt op termijn `crs` als
    /// top-level. Bevat EPSG + naam + optionele map-init + alle
    /// base/overlay lagen zodat de kaart-tab exact te reproduceren
    /// is uit één bestand. `crs` blijft als deprecated mirror voor
    /// 0.2-loaders; nieuwe code mag uit `gis` lezen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gis: Option<GisMetadata>,
    /// Deliverable representatie (sinds 0.3) — de huidige stand van
    /// de tekening als 2D IFC-stijl object-graph, los van de muteer-
    /// bare `tekening`-state. Bedoeld voor downstream-tools die het
    /// bestand willen lezen zonder app-state te begrijpen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliverable: Option<Deliverable>,
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

// ────────────────────────────────────────────────────────────────────
// schema 0.3 — GIS metadata + deliverable
// ────────────────────────────────────────────────────────────────────

/// GIS-metadata: alle info die de map-view nodig heeft om de kaart-
/// tab te reproduceren. Sinds ifcgis-0.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GisMetadata {
    /// EPSG-code van het werk-CRS. Standaard 28992 (Amersfoort / RD
    /// New).
    pub epsg: u32,
    /// Menselijk leesbare CRS-naam (b.v. "Amersfoort / RD New").
    pub name: String,
    /// Optionele init-positie voor de kaart-tab. Als `None` bepaalt
    /// de frontend zelf een fit-bounds o.b.v. de CPT-/Bore-locaties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<ViewCenter>,
    /// Alle base- en overlay-lagen. Volgorde is rendering-order
    /// (eerste = onderaan).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<GisLayer>,
}

impl Default for GisMetadata {
    fn default() -> Self {
        Self {
            epsg: 28992,
            name: "Amersfoort / RD New".to_string(),
            center: None,
            layers: Vec::new(),
        }
    }
}

/// Eén kaartlaag (base of overlay) met genoeg metadata om hem zonder
/// app-defaults opnieuw aan te roepen. Url/layer_name/style zijn de
/// service-parameters; enabled/opacity is de UI-state op het moment
/// van saven.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GisLayer {
    /// Stabiele id (b.v. "topo", "luchtfoto-actueel", "kadaster").
    pub id: String,
    /// User-readable label voor in de layer-switcher.
    pub label: String,
    /// "base" of "overlay" — sturend voor de UI-grouping en voor de
    /// mutex-regel dat er altijd één base actief is.
    pub group: String,
    /// "wmts" | "wms" | "wfs" | "tile" — bepaalt de Leaflet-loader.
    pub kind: String,
    /// Template-URL (voor tile/wmts met {z}/{x}/{y}) of base WMS/WFS
    /// endpoint.
    pub url: String,
    /// Voor WMS: LAYERS=...; voor WFS: typeName.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_name: Option<String>,
    /// STYLES= voor WMS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Of de laag aanstaat op het moment van saven.
    pub enabled: bool,
    /// Layer-opacity 0.0..1.0.
    pub opacity: f32,
    /// © PDOK / Kadaster / etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_zoom: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_zoom: Option<u32>,
}

/// 2D IFC-stijl deliverable — de huidige stand van de tekening als
/// platte object-lijst. Bewust *naast* `tekening` (editor-state)
/// zodat downstream-tools een stabiele lees-representatie hebben
/// zonder app-state te kennen. Voor IFC4x3 mapt dit op
/// `IfcDrawingSheet` als top-level container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deliverable {
    /// IFC-class. Voor 2D drawings doorgaans "IfcDrawingSheet" of
    /// "IfcSheet"; voor IFC4x3 kan ook "IfcAnnotation" als top-level.
    pub ifc_class: String,
    /// 22-char IFC-stijl GUID. Frontend genereert deze (cpt-core
    /// heeft geen uuid-dep).
    pub guid: String,
    /// Doorgaans projectnaam + " — Situatietekening".
    pub name: String,
    /// "A2" / "A3" / etc.
    pub paper_size: String,
    /// "landscape" / "portrait".
    pub orientation: String,
    /// Print-schaal als N (b.v. 500, 1000, 2000 voor 1:N).
    pub scale: u32,
    /// EPSG van het werk-CRS waarin de RD-coördinaten leven.
    pub crs_epsg: u32,
    /// Centrum van de viewport bij het opslaan.
    pub view_center: ViewCenter,
    /// Alle geplaatste objecten als IFC-stijl annotations. Flat —
    /// `ifc_class` per element zegt wat het is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<IfcAnnotation>,
    /// Title-block velden zoals geprint op het papier (kopie van het
    /// top-level `title_block` op het moment van saven — staat hier
    /// óók in zodat de deliverable zelfcontained is).
    pub title_block: TitleBlock,
    /// Ids van GIS-layers die actief waren bij het saven. De volledige
    /// layer-definities staan in `gis.layers`; hier alleen de
    /// referenties zodat de deliverable weet wat er zichtbaar was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_layer_ids: Vec<String>,
}

/// Eén IFC-stijl annotation. Open-typed zodat we niet twintig
/// varianten van de struct hoeven onderhouden — `ifc_class` zegt
/// wat het is, `geometry` en `properties` houden de type-specifieke
/// payload.
///
/// Toegestane `ifc_class` waarden (IFC4x3):
///   * "IfcAnnotation/Sondering" — CPT marker
///   * "IfcAnnotation/Boring" — BHR marker
///   * "IfcAnnotation/Raster" — sonderingsraster
///   * "IfcAnnotation/Line" — vrije lijn
///   * "IfcAnnotation/Dimension" — maatlijn
///   * "IfcAnnotation/CoordTag" — RD-coördinaat label
///   * "IfcGeographicElement/Overlay" — image/svg/pdf overlay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfcAnnotation {
    /// IFC4x3 class — zie module-doc voor toegestane waarden.
    pub ifc_class: String,
    /// 22-char IFC GUID. Frontend genereert.
    pub guid: String,
    /// User-readable label (b.v. CPT-id).
    pub name: String,
    /// Geometry in lat/lon (WGS84). JSON-array van [lat, lon]
    /// tuples. Punt = 1 paar, lijn = 2, polygoon = ≥3.
    pub geometry: serde_json::Value,
    /// Type-specifieke extra's. Voor sondering: kleefmeting (bool).
    /// Voor raster: rows, cols, spacing_x, spacing_y, rotation.
    /// Voor overlay: src (data-URL), width_meters. Bewust open-typed.
    #[serde(default)]
    pub properties: serde_json::Value,
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
        // 0.3-velden: dit pad is de "korte" save zonder GIS/
        // deliverable. Frontend gebruikt save_project_ifcgis_full
        // (serde_json::Value) wanneer ze die wél willen meegeven.
        gis: None,
        deliverable: None,
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
        // Schema-versie liep mee naar 0.3 toen `gis` en `deliverable`
        // erbij kwamen. Een 0.2-loader negeert die nieuwe velden
        // (serde tolereert unknown fields by default niet, maar het
        // ProjectFile-struct heeft ze niet — dus dat zou wel falen).
        // Forward-compat is geregeld doordat alle nieuwe velden in
        // het schema `#[serde(default, skip_serializing_if = ...)]`
        // hebben zodat oude 0.2-input nog laadt op een 0.3-loader.
        assert!(
            json.contains("\"schema\": \"ifcgis-0.3\""),
            "expected schema 0.3 in output, got: {json}"
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

    // ─── schema 0.3 tests ──────────────────────────────────────────

    fn sample_gis() -> GisMetadata {
        GisMetadata {
            epsg: 28992,
            name: "Amersfoort / RD New".into(),
            center: Some(ViewCenter { lat: 52.0907, lon: 5.1214, zoom: 13.0 }),
            layers: vec![
                GisLayer {
                    id: "topo".into(),
                    label: "Topografie (BRT)".into(),
                    group: "base".into(),
                    kind: "wmts".into(),
                    url: "https://service.pdok.nl/brt/achtergrondkaart/wmts/v2_0/{layer}/{tileMatrixSet}/{z}/{x}/{y}.png".into(),
                    layer_name: Some("standaard".into()),
                    style: None,
                    enabled: true,
                    opacity: 1.0,
                    attribution: Some("© Kadaster / PDOK".into()),
                    min_zoom: Some(6),
                    max_zoom: Some(19),
                },
                GisLayer {
                    id: "kadaster".into(),
                    label: "Kadastrale percelen".into(),
                    group: "overlay".into(),
                    kind: "wms".into(),
                    url: "https://service.pdok.nl/kadaster/kadastralekaart/wms/v5_0".into(),
                    layer_name: Some("Perceel".into()),
                    style: Some("default".into()),
                    enabled: false,
                    opacity: 0.6,
                    attribution: Some("© Kadaster".into()),
                    min_zoom: None,
                    max_zoom: None,
                },
            ],
        }
    }

    fn sample_deliverable() -> Deliverable {
        Deliverable {
            ifc_class: "IfcDrawingSheet".into(),
            guid: "0123456789abcdefghijkl".into(), // 22-char placeholder GUID
            name: "Test project — Situatietekening".into(),
            paper_size: "A2".into(),
            orientation: "landscape".into(),
            scale: 500,
            crs_epsg: 28992,
            view_center: ViewCenter { lat: 52.0907, lon: 5.1214, zoom: 16.0 },
            annotations: vec![
                IfcAnnotation {
                    ifc_class: "IfcAnnotation/Sondering".into(),
                    guid: "abcd1234efgh5678ijkl90".into(),
                    name: "S01".into(),
                    geometry: serde_json::json!([[52.0907, 5.1214]]),
                    properties: serde_json::json!({ "kleefmeting": true }),
                },
                IfcAnnotation {
                    ifc_class: "IfcAnnotation/Line".into(),
                    guid: "mnop1234qrst5678uvwx90".into(),
                    name: "Doorsnede A-A".into(),
                    geometry: serde_json::json!([[52.0900, 5.1200], [52.0915, 5.1230]]),
                    properties: serde_json::json!({}),
                },
            ],
            title_block: TitleBlock {
                project: "Test project".into(),
                project_number: "2026-001".into(),
                drawing_number: "T-001".into(),
                scale: "1:500".into(),
                date: "2026-05-18".into(),
                ..Default::default()
            },
            active_layer_ids: vec!["topo".into()],
        }
    }

    #[test]
    fn round_trip_ifcgis_0_3_full() {
        // Bouw een 0.3-ProjectFile met alle nieuwe secties gevuld
        // en bevestig dat ze allemaal overleven door save → load.
        let file = ProjectFile {
            header: Header::new("Open Geotechniek Studio"),
            project: sample_project(),
            cpts: vec![sample_cpt("S01")],
            bores: Vec::new(),
            crs: Crs::default(),
            tekening: None,
            title_block: Some(TitleBlock {
                project: "Test project".into(),
                project_number: "2026-001".into(),
                ..Default::default()
            }),
            gis: Some(sample_gis()),
            deliverable: Some(sample_deliverable()),
        };
        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"schema\": \"ifcgis-0.3\""));
        assert!(json.contains("\"gis\""), "expected gis section in output");
        assert!(json.contains("\"deliverable\""), "expected deliverable section");
        assert!(json.contains("\"IfcDrawingSheet\""));
        assert!(json.contains("\"topo\""));

        let back = load(&json).unwrap();
        let gis = back.gis.as_ref().expect("gis should round-trip");
        assert_eq!(gis.epsg, 28992);
        assert_eq!(gis.layers.len(), 2);
        assert_eq!(gis.layers[0].id, "topo");
        assert!(gis.layers[0].enabled);
        assert_eq!(gis.layers[1].id, "kadaster");
        assert!(!gis.layers[1].enabled);
        assert_eq!(gis.layers[1].opacity, 0.6);
        assert_eq!(gis.layers[0].layer_name.as_deref(), Some("standaard"));

        let deliv = back.deliverable.as_ref().expect("deliverable should round-trip");
        assert_eq!(deliv.ifc_class, "IfcDrawingSheet");
        assert_eq!(deliv.scale, 500);
        assert_eq!(deliv.annotations.len(), 2);
        assert_eq!(deliv.annotations[0].ifc_class, "IfcAnnotation/Sondering");
        assert_eq!(deliv.annotations[0].name, "S01");
        assert_eq!(deliv.active_layer_ids, vec!["topo".to_string()]);
    }

    #[test]
    fn loads_0_2_file_as_0_3() {
        // Simuleer een echt 0.2-bestand zonder `gis` en `deliverable`
        // velden — de 0.3-loader moet dat nog steeds accepteren en
        // de nieuwe velden op None / default zetten.
        let json_0_2 = r#"{
            "header": {
                "schema": "ifcgis-0.2",
                "originating_system": "Open Geotechniek Studio",
                "timestamp": "2026-05-15T10:00:00+00:00"
            },
            "project": {
                "type": "OpenGeoProject",
                "title": "Legacy 0.2 project",
                "client": "ACME bv",
                "location": "Utrecht",
                "project_number": "2024-099",
                "author": "OGS",
                "date": "2024-12-01"
            },
            "cpts": [],
            "crs": { "epsg": 28992, "name": "Amersfoort / RD New" }
        }"#;
        let back = load(json_0_2).expect("0.2 file must still load on 0.3 loader");
        assert_eq!(back.header.schema, "ifcgis-0.2");
        assert_eq!(back.project.title, "Legacy 0.2 project");
        assert!(back.gis.is_none(), "gis defaults to None for 0.2 files");
        assert!(back.deliverable.is_none(), "deliverable defaults to None");
        assert!(back.bores.is_empty());
        assert_eq!(back.crs.epsg, 28992);
    }

    #[test]
    fn deliverable_annotation_geometry_round_trips() {
        // Een IfcAnnotation met een lat/lon-array als geometry moet
        // bit-by-bit terugkomen — de waarden zijn open JSON, dus
        // we vergelijken via serde_json::Value-equality.
        let ann = IfcAnnotation {
            ifc_class: "IfcAnnotation/Line".into(),
            guid: "geomtest1234567890abcd".into(),
            name: "Profielas".into(),
            geometry: serde_json::json!([
                [52.37000, 4.89000],
                [52.37050, 4.89100],
                [52.37100, 4.89200]
            ]),
            properties: serde_json::json!({
                "kind": "section",
                "label": "A-A'"
            }),
        };
        let json = serde_json::to_string(&ann).unwrap();
        let back: IfcAnnotation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ifc_class, ann.ifc_class);
        assert_eq!(back.guid, ann.guid);
        assert_eq!(back.name, ann.name);
        assert_eq!(back.geometry, ann.geometry);
        assert_eq!(back.properties, ann.properties);

        // Concreet sanity-check op de geometry-vorm: het is een array
        // van 3 tuples, ieder met 2 floats.
        let arr = back.geometry.as_array().expect("geometry should be array");
        assert_eq!(arr.len(), 3);
        let first = arr[0].as_array().expect("each entry is a tuple");
        assert_eq!(first.len(), 2);
        assert!((first[0].as_f64().unwrap() - 52.37000).abs() < 1e-9);
        assert!((first[1].as_f64().unwrap() - 4.89000).abs() < 1e-9);
    }
}
