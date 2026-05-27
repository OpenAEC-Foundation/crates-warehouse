//! `.ifcgis` — Open GEO Studio project file format.
//!
//! **Wire-format = IFCX (IFC5 alpha).** On disk, an `.ifcgis` is a real
//! IFCX JSON document: an IFC4x3-style header followed by a flat `data`
//! array of IFC entities (`type` + `GlobalId` + IFC attributes), cross-
//! linked via `#id` reference strings.
//!
//! The internal app-state is still the rich `ProjectFile` struct so the
//! rest of the codebase doesn't have to think in IFC entities. Conversion
//! happens at the file boundary:
//!
//! * [`to_ifcx_json`] — `ProjectFile` → IFCX JSON string
//! * [`from_ifcx_json`] — IFCX JSON string → `ProjectFile`
//!
//! [`save_full`] always writes IFCX. [`load`] sniffs the input: a top-
//! level `data` array (or an IFC4X3 `schemaIdentifiers` entry) routes to
//! IFCX parsing; legacy `ifcgis-0.1/0.2/0.3` files still load via the
//! original schema for forward-compat with existing user files.
//!
//! For pragmatism we don't pretend to implement the full IFC4x3 schema:
//! when an IFC class isn't a perfect fit (geotechnical strata, GIS layers,
//! drawing markers) we fall back to `IfcAnnotation` / `IfcProxy` plus a
//! property-set carrying the original payload — documented per call site.
//! The structural conformance (header + data + typed entities + `#id`
//! references) is the contract, not 1:1 IFC-spec strictness.
//!
//! File extension: `.ifcgis`
//! Media type: `application/x.ifcgis+json`

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

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
const SCHEMA_VERSION: &str = "ifcgis-0.4";

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
    /// Geotechnische + constructieve berekeningen — sinds 0.4.
    /// Per-module input JSON-payload, herrekend bij open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculations: Vec<CalculationDef>,
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

/// Geotechnische / constructieve berekening — sinds ifcgis-0.4.
///
/// Eén entry per door de gebruiker aangemaakte berekening; het
/// `module_id` selecteert welke calculator de `input`-payload
/// interpreteert. `input` is open JSON zodat ieder module-team zijn
/// eigen schema kan bijhouden zonder cpt-core bij elke iteratie te
/// hoeven hertypen. Resultaten worden niet opgeslagen — die rekenen
/// we opnieuw uit bij open (cheap + altijd vers t.o.v. de huidige
/// CPT/Bore-set).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationDef {
    pub id: String,
    pub module_id: String,
    pub name: String,
    #[serde(default)]
    pub input: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpt_refs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bore_refs: Option<Vec<String>>,
}

/// Serialise a project (metadata + CPTs + optional bores/tekening/
/// title_block) to a pretty-printed `.ifcgis` JSON string.
pub fn save(project: ProjectInfo, cpts: Vec<Cpt>) -> Result<String, CptError> {
    save_full(project, cpts, Vec::new(), None, None)
}

/// Full save — including bores, tekening-layout, title-block.
/// `save()` blijft als shortcut voor backward compat met bestaande
/// Tauri-commands die alleen CPTs hebben.
///
/// **Wire-format**: emits real IFCX (IFC5 alpha) JSON via
/// [`to_ifcx_json`]. The in-memory `ProjectFile` is the contract with
/// the rest of the app — the IFC mapping is an output-only concern.
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
        // 0.4-veld: berekeningen — leeg op de korte save-pad.
        calculations: Vec::new(),
    };
    to_ifcx_json(&file)
}

/// Parse a `.ifcgis` JSON file into the project model.
///
/// Detects which on-disk format the input is:
///   * **IFCX** (top-level `data` array, IFC4x3-style `header.schemaIdentifiers`)
///     → routes to [`from_ifcx_json`]
///   * **Legacy `ifcgis-0.1` / `0.2` / `0.3`** (top-level `header.schema`
///     starting with `ifcgis-`, plus `project` / `cpts` / ...) → parsed
///     directly into [`ProjectFile`] for forward-compat with existing
///     user files
pub fn load(text: &str) -> Result<ProjectFile, CptError> {
    // Cheap sniff: parse as raw Value first so we can branch without
    // committing to either format.
    let raw: Value = serde_json::from_str(text)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis parse: {e}")))?;

    if looks_like_ifcx(&raw) {
        return from_ifcx_json(text);
    }

    // Legacy route — schema 0.1/0.2/0.3 (rich struct on disk).
    let file: ProjectFile = serde_json::from_value(raw)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis parse: {e}")))?;
    if !file.header.schema.starts_with("ifcgis-") {
        return Err(CptError::InvalidGef(format!(
            "unrecognized schema '{}' (expected ifcgis-* or IFCX)",
            file.header.schema
        )));
    }
    Ok(file)
}

/// Heuristic: is this JSON IFCX (real IFC5-alpha shape) or legacy
/// `ifcgis-0.x` (our old rich struct)?
///
/// IFCX has a top-level `data: [...]` array of entities. Legacy files
/// have `project`, `cpts`, etc. The sniff prefers IFCX when both a
/// `data` array exists AND it contains entities with `type` fields —
/// that way we never false-positive on legacy files that happen to have
/// a stray top-level `data` field.
fn looks_like_ifcx(raw: &Value) -> bool {
    let Some(obj) = raw.as_object() else { return false };
    // Strong signal: header.schemaIdentifiers contains "IFC4X3..." or
    // "IFCX..." → definitely IFCX shape.
    if let Some(hdr) = obj.get("header").and_then(|h| h.as_object()) {
        if let Some(ids) = hdr.get("schemaIdentifiers").and_then(|v| v.as_array()) {
            if ids
                .iter()
                .filter_map(|v| v.as_str())
                .any(|s| s.starts_with("IFC"))
            {
                return true;
            }
        }
    }
    // Otherwise require a non-empty `data` array of typed entities,
    // and the absence of the legacy `project` key (which is always
    // present on `ifcgis-0.x`).
    let has_data_array = obj
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .any(|e| e.get("type").and_then(|t| t.as_str()).is_some())
        })
        .unwrap_or(false);
    let has_legacy_project = obj
        .get("project")
        .map(|p| p.is_object())
        .unwrap_or(false);
    has_data_array && !has_legacy_project
}

// ────────────────────────────────────────────────────────────────────
// IFCX (IFC5 alpha) wire-format
// ────────────────────────────────────────────────────────────────────

/// The IFC4x3 schema identifier we advertise. Real IFC tooling reads
/// this to choose its property dictionary.
const IFC_SCHEMA: &str = "IFC4X3_ADD2";
/// IFCX writer signature — bumped when the mapping changes meaningfully.
const PREPROCESSOR_VERSION: &str = "Open Geotechniek Studio 0.3 (IFCX writer)";
/// Stable namespace seed for v5 GUIDs. Prefix every seed with this so a
/// project_title="X" never collides with a cpt id="X".
const GUID_NAMESPACE: &str = "open-geotechniek-studio/v1";

// ─── reference-id helpers ───────────────────────────────────────────
//
// IFCX cross-links entities via `#id` strings (STEP convention, JSON
// flavour). We assign stable ids per category so the output is
// reproducible: project=#project, site=#site, units=#units, ctx3d=
// #ctx-3d, etc. Per-CPT/Bore/layer ids use the entity's user id as
// suffix to stay readable for humans diffing files in git.

fn ref_id(prefix: &str, key: &str) -> String {
    // Slugify the key so the resulting id stays valid in JSON & on
    // disk — strip everything that isn't [A-Za-z0-9_-].
    let clean: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("#{prefix}-{clean}")
}

/// Convert a [`ProjectFile`] into an IFCX (IFC5 alpha) JSON string.
///
/// Output shape:
/// ```json
/// {
///   "header": { /* IFC4x3-style metadata */ },
///   "data":   [ /* flat array of IFC entities */ ]
/// }
/// ```
///
/// The legacy `ifcgis-0.3` payload is also embedded verbatim under
/// `metadata.openGeoStudio` in the header — this is what
/// [`from_ifcx_json`] uses to round-trip the rich `ProjectFile` back
/// without lossy IFC re-interpretation. (Pure IFC consumers ignore
/// unknown header keys, so this is harmless.)
pub fn to_ifcx_json(file: &ProjectFile) -> Result<String, CptError> {
    let mut data: Vec<Value> = Vec::new();

    // ─── IfcProject ─────────────────────────────────────────────
    let proj_guid = ifc_guid(&format!("{GUID_NAMESPACE}:project:{}", file.project.title));
    let owner_ref = ref_id("ownerhistory", "default");
    let ctx_3d_ref = ref_id("ctx", "3d");
    let ctx_2d_ref = ref_id("ctx", "2d");
    let units_ref = ref_id("units", "default");
    data.push(json!({
        "type": "IfcProject",
        "id": "#project",
        "GlobalId": proj_guid,
        "Name": file.project.title,
        "Description": file.project.client,
        "LongName": file.project.location,
        "Phase": "Ontwerp",
        "ObjectType": "OpenGeoProject",
        "OwnerHistory": owner_ref,
        "RepresentationContexts": [ctx_3d_ref, ctx_2d_ref],
        "UnitsInContext": units_ref,
    }));

    // ─── IfcOwnerHistory (minimal) ──────────────────────────────
    data.push(json!({
        "type": "IfcOwnerHistory",
        "id": "#ownerhistory-default",
        "OwningUser": file.project.author,
        "OwningApplication": "Open Geotechniek Studio",
        "ChangeAction": "ADDED",
        "CreationDate": file.header.timestamp,
    }));

    // ─── IfcUnitAssignment + IfcSIUnit(s) ───────────────────────
    let unit_len_ref = ref_id("unit", "length");
    let unit_pres_ref = ref_id("unit", "pressure");
    let unit_plane_ref = ref_id("unit", "plane");
    data.push(json!({
        "type": "IfcUnitAssignment",
        "id": "#units-default",
        "Units": [unit_len_ref, unit_pres_ref, unit_plane_ref],
    }));
    data.push(json!({
        "type": "IfcSIUnit",
        "id": "#unit-length",
        "UnitType": "LENGTHUNIT",
        "Name": "METRE",
    }));
    data.push(json!({
        "type": "IfcSIUnit",
        "id": "#unit-pressure",
        "UnitType": "PRESSUREUNIT",
        "Prefix": "MEGA",
        "Name": "PASCAL",
    }));
    data.push(json!({
        "type": "IfcSIUnit",
        "id": "#unit-plane",
        "UnitType": "PLANEANGLEUNIT",
        "Name": "RADIAN",
    }));

    // ─── IfcGeometricRepresentationContext (Model / Plan) ───────
    data.push(json!({
        "type": "IfcGeometricRepresentationContext",
        "id": "#ctx-3d",
        "ContextIdentifier": "Model",
        "ContextType": "Model",
        "CoordinateSpaceDimension": 3,
        "Precision": 1.0e-5,
    }));
    data.push(json!({
        "type": "IfcGeometricRepresentationContext",
        "id": "#ctx-2d",
        "ContextIdentifier": "Plan",
        "ContextType": "Plan",
        "CoordinateSpaceDimension": 2,
        "Precision": 1.0e-5,
    }));

    // ─── IfcProjectedCRS + IfcMapConversion ─────────────────────
    let crs = &file.crs;
    let crs_ref = ref_id("crs", &format!("epsg-{}", crs.epsg));
    data.push(json!({
        "type": "IfcProjectedCRS",
        "id": crs_ref.trim_start_matches('#'),
        "Name": format!("EPSG:{}", crs.epsg),
        "Description": crs.name,
        "GeodeticDatum": "Amersfoort",
        "MapProjection": "RD New",
        "MapUnit": unit_len_ref,
    }));
    data.push(json!({
        "type": "IfcMapConversion",
        "id": "#mapconv-site",
        "SourceCRS": ctx_3d_ref,
        "TargetCRS": crs_ref,
        "Eastings": 0.0,
        "Northings": 0.0,
        "OrthogonalHeight": 0.0,
        "XAxisAbscissa": 1.0,
        "XAxisOrdinate": 0.0,
        "Scale": 1.0,
    }));

    // ─── IfcSite + IfcPostalAddress ─────────────────────────────
    let site_guid = ifc_guid(&format!("{GUID_NAMESPACE}:site:{}", file.project.location));
    let addr_ref = ref_id("addr", "site");
    let (ref_lat, ref_lon) = file
        .gis
        .as_ref()
        .and_then(|g| g.center.as_ref())
        .map(|c| (c.lat, c.lon))
        .unwrap_or((52.0, 5.0));
    data.push(json!({
        "type": "IfcSite",
        "id": "#site",
        "GlobalId": site_guid,
        "Name": "Projectsite",
        "Description": file.project.location,
        "CompositionType": "ELEMENT",
        "RefLatitude": decimal_degrees_to_dms(ref_lat),
        "RefLongitude": decimal_degrees_to_dms(ref_lon),
        "RefElevation": 0.0,
        "SiteAddress": addr_ref,
    }));
    data.push(json!({
        "type": "IfcPostalAddress",
        "id": "#addr-site",
        "Purpose": "SITE",
        "AddressLines": [file.project.location.clone()],
        "Town": file.project.location,
        "Country": "Nederland",
    }));

    // ─── IfcBorehole per CPT ────────────────────────────────────
    //
    // IFC4x3 has IfcBorehole with PredefinedType including
    // `GEOTECHNICAL_DRILLING_INVESTIGATION`. CPT (cone penetration
    // test) doesn't map perfectly; we use the borehole entity with
    // a custom ObjectType="ConePenetrationTest" plus a property-set
    // carrying the measurement vectors. Strict IFC consumers can
    // read the borehole as a generic geotechnical investigation;
    // OGS-aware consumers see the full CPT payload.
    for cpt in &file.cpts {
        emit_cpt_entities(&mut data, cpt, &crs_ref);
    }

    // ─── IfcBorehole per BHR (opaque JSON) ──────────────────────
    //
    // The bores are stored as `serde_json::Value` (the frontend owns
    // their schema). We surface their id + position as a real IFC
    // borehole and carry the full original payload in a property-set
    // so round-trip is lossless.
    for (idx, bore) in file.bores.iter().enumerate() {
        emit_bore_entities(&mut data, bore, idx);
    }

    // ─── GIS layers as IfcGeographicElement ─────────────────────
    if let Some(gis) = &file.gis {
        for layer in &gis.layers {
            let lguid = ifc_guid(&format!("{GUID_NAMESPACE}:gislayer:{}", layer.id));
            let lid = ref_id("gislayer", &layer.id);
            data.push(json!({
                "type": "IfcGeographicElement",
                "id": lid.trim_start_matches('#'),
                "GlobalId": lguid,
                "Name": layer.label,
                "Description": layer.attribution.clone().unwrap_or_default(),
                "ObjectType": format!("GISLayer/{}", layer.group),
                "PredefinedType": "USERDEFINED",
                "ContainedInStructure": "#site",
            }));
            // Property-set with the layer's wire-format parameters.
            let pset_id = ref_id("pset-gislayer", &layer.id);
            let mut props = vec![
                json_str_prop("Kind", &layer.kind),
                json_str_prop("Url", &layer.url),
                json_bool_prop("Enabled", layer.enabled),
                json_num_prop("Opacity", layer.opacity as f64),
            ];
            if let Some(ln) = &layer.layer_name {
                props.push(json_str_prop("LayerName", ln));
            }
            if let Some(st) = &layer.style {
                props.push(json_str_prop("Style", st));
            }
            if let Some(mn) = layer.min_zoom {
                props.push(json_num_prop("MinZoom", mn as f64));
            }
            if let Some(mx) = layer.max_zoom {
                props.push(json_num_prop("MaxZoom", mx as f64));
            }
            data.push(json!({
                "type": "IfcPropertySet",
                "id": pset_id.trim_start_matches('#'),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:gislayer-pset:{}", layer.id)),
                "Name": "OpenGeoStudio_GISLayer",
                "HasProperties": props,
            }));
            data.push(json!({
                "type": "IfcRelDefinesByProperties",
                "id": format!("rel-{}", pset_id.trim_start_matches('#')),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:gislayer-rel:{}", layer.id)),
                "RelatedObjects": [lid],
                "RelatingPropertyDefinition": pset_id,
            }));
        }
    }

    // ─── Deliverable → IfcSheet + IfcAnnotation per element ─────
    if let Some(deliv) = &file.deliverable {
        let sheet_id = "#sheet-deliverable";
        data.push(json!({
            "type": "IfcSheet",
            "id": sheet_id.trim_start_matches('#'),
            "GlobalId": deliv.guid.clone(),
            "Name": deliv.name,
            "ObjectType": deliv.ifc_class,
            "PredefinedType": "SHEET",
            "ContainedInStructure": "#site",
        }));
        // Sheet metadata pset (paper, scale, orientation).
        let sheet_pset_id = "#pset-sheet-deliverable";
        data.push(json!({
            "type": "IfcPropertySet",
            "id": sheet_pset_id.trim_start_matches('#'),
            "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:sheet-pset:{}", deliv.guid)),
            "Name": "OpenGeoStudio_SheetMeta",
            "HasProperties": [
                json_str_prop("PaperSize", &deliv.paper_size),
                json_str_prop("Orientation", &deliv.orientation),
                json_num_prop("Scale", deliv.scale as f64),
                json_num_prop("CrsEpsg", deliv.crs_epsg as f64),
                json_num_prop("ViewCenterLat", deliv.view_center.lat),
                json_num_prop("ViewCenterLon", deliv.view_center.lon),
                json_num_prop("ViewCenterZoom", deliv.view_center.zoom),
            ],
        }));
        data.push(json!({
            "type": "IfcRelDefinesByProperties",
            "id": "rel-pset-sheet-deliverable",
            "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:sheet-rel:{}", deliv.guid)),
            "RelatedObjects": [sheet_id],
            "RelatingPropertyDefinition": sheet_pset_id,
        }));
        // One IfcAnnotation per drawing element.
        for (idx, ann) in deliv.annotations.iter().enumerate() {
            let ann_id = format!("#annotation-{idx}");
            let ann_pred = annotation_predefined_type(&ann.ifc_class);
            data.push(json!({
                "type": "IfcAnnotation",
                "id": ann_id.trim_start_matches('#'),
                "GlobalId": ann.guid.clone(),
                "Name": ann.name,
                "ObjectType": ann.ifc_class,
                "PredefinedType": ann_pred,
                "ContainedInStructure": sheet_id,
            }));
            let ann_pset_id = format!("#pset-annotation-{idx}");
            data.push(json!({
                "type": "IfcPropertySet",
                "id": ann_pset_id.trim_start_matches('#'),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:ann-pset:{}", ann.guid)),
                "Name": "OpenGeoStudio_Annotation",
                "HasProperties": [
                    json_str_prop("IfcClass", &ann.ifc_class),
                    json_raw_prop("Geometry", ann.geometry.clone()),
                    json_raw_prop("Properties", ann.properties.clone()),
                ],
            }));
            data.push(json!({
                "type": "IfcRelDefinesByProperties",
                "id": format!("rel-pset-annotation-{idx}"),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:ann-rel:{}", ann.guid)),
                "RelatedObjects": [ann_id],
                "RelatingPropertyDefinition": ann_pset_id,
            }));
        }
        // Active layer ids — capture as a property-set on the sheet
        // so the deliverable knows which GIS layers were visible.
        if !deliv.active_layer_ids.is_empty() {
            let ids: Vec<Value> = deliv
                .active_layer_ids
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect();
            data.push(json!({
                "type": "IfcPropertySet",
                "id": "pset-sheet-active-layers",
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:sheet-active-layers:{}", deliv.guid)),
                "Name": "OpenGeoStudio_ActiveLayers",
                "HasProperties": [
                    json_raw_prop("ActiveLayerIds", Value::Array(ids)),
                ],
            }));
        }
    }

    // ─── Title block → IfcDocumentInformation ───────────────────
    if let Some(tb) = &file.title_block {
        data.push(json!({
            "type": "IfcDocumentInformation",
            "id": "doc-titleblock",
            "Identification": "TitleBlock",
            "Name": tb.project.clone(),
            "Description": format!("{} — {}", tb.project_number, tb.drawing_number),
            "Purpose": "TITLE_BLOCK",
            "Revision": tb.version.clone(),
            "DocumentOwner": tb.drawn_by.clone(),
            "Editors": [tb.checked_by.clone()],
            "CreationTime": tb.date.clone(),
            "Scope": tb.scale.clone(),
            "IntendedUse": tb.address.clone(),
        }));
    }

    // ─── Tekening (editor-state) → IfcAnnotation entities ───────
    //
    // The mutable editor state lives alongside the deliverable
    // (which is the immutable snapshot). We emit each marker / raster
    // / line as a real IfcAnnotation so they're inspectable by IFC
    // tooling — the round-trip back to TekeningLayout happens via the
    // embedded openGeoStudio block in the header (see below).
    if let Some(tek) = &file.tekening {
        for m in &tek.markers {
            let aid = ref_id("marker", &m.id);
            data.push(json!({
                "type": "IfcAnnotation",
                "id": aid.trim_start_matches('#'),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:marker:{}", m.id)),
                "Name": m.id.clone(),
                "ObjectType": format!("Marker/{}", m.kind),
                "PredefinedType": "USERDEFINED",
                "ContainedInStructure": "#site",
            }));
        }
        for r in &tek.rasters {
            let aid = ref_id("raster", &r.id);
            data.push(json!({
                "type": "IfcAnnotation",
                "id": aid.trim_start_matches('#'),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:raster:{}", r.id)),
                "Name": r.id.clone(),
                "ObjectType": "Marker/Raster",
                "PredefinedType": "USERDEFINED",
                "ContainedInStructure": "#site",
            }));
        }
        for ln in &tek.lines {
            let aid = ref_id("line", &ln.id);
            data.push(json!({
                "type": "IfcAnnotation",
                "id": aid.trim_start_matches('#'),
                "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:line:{}", ln.id)),
                "Name": ln.id.clone(),
                "ObjectType": format!("Line/{}", ln.kind),
                "PredefinedType": if ln.kind == "dimension" { "DIMENSION" } else { "USERDEFINED" },
                "ContainedInStructure": "#site",
            }));
        }
    }

    // ─── Header (IFC4x3-style) + embedded openGeoStudio block ────
    //
    // The header has the standard IFC4x3 file-description shape so
    // generic IFCX consumers can read it. The `openGeoStudio` key
    // carries the full legacy `ProjectFile` payload (serde_json) so
    // [`from_ifcx_json`] can reconstruct the rich struct losslessly
    // without re-interpreting the IFC entities.
    let legacy_payload = serde_json::to_value(file)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis legacy payload: {e}")))?;
    let header = json!({
        "fileDescription": ["ViewDefinition[GeotechnicalView]"],
        "fileName": format!("{}.ifcgis", safe_filename(&file.project.title)),
        "timeStamp": file.header.timestamp,
        "author": [file.project.author.clone()],
        "organization": ["OpenAEC Foundation"],
        "preprocessorVersion": PREPROCESSOR_VERSION,
        "originatingSystem": file.header.originating_system,
        "authorization": "",
        "schemaIdentifiers": [IFC_SCHEMA],
        "metadata": {
            "openGeoStudio": legacy_payload,
        }
    });

    let doc = json!({
        "header": header,
        "data": data,
    });
    serde_json::to_string_pretty(&doc)
        .map_err(|e| CptError::InvalidGef(format!("ifcgis ifcx serialize: {e}")))
}

/// Inverse of [`to_ifcx_json`]: parse an IFCX JSON string back into a
/// [`ProjectFile`].
///
/// **Lossless path**: when the IFCX document was produced by us, the
/// header carries an embedded `metadata.openGeoStudio` block that *is*
/// the legacy `ProjectFile` payload. We deserialise that directly.
///
/// **Lossy path** (third-party IFCX input): if the embedded block is
/// missing, we reconstruct what we can from the IFC entities —
/// IfcProject → ProjectInfo, IfcBorehole → minimal Cpt, etc. This is
/// best-effort; users who hand-author IFCX without our header will
/// lose tekening / GIS state.
pub fn from_ifcx_json(text: &str) -> Result<ProjectFile, CptError> {
    let raw: Value = serde_json::from_str(text)
        .map_err(|e| CptError::InvalidGef(format!("ifcx parse: {e}")))?;

    // Fast path — our own writer embeds the legacy struct verbatim.
    if let Some(embedded) = raw
        .get("header")
        .and_then(|h| h.get("metadata"))
        .and_then(|m| m.get("openGeoStudio"))
    {
        let mut file: ProjectFile = serde_json::from_value(embedded.clone())
            .map_err(|e| CptError::InvalidGef(format!("ifcx embedded payload: {e}")))?;
        // Preserve the IFCX schema marker on the header so downstream
        // code can tell this file went through the IFCX pipeline.
        if !file.header.schema.starts_with("ifcgis-") {
            file.header.schema = SCHEMA_VERSION.into();
        }
        return Ok(file);
    }

    // Slow path — reconstruct from raw IFC entities. Third-party
    // input; preserve whatever we can.
    let data = raw
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| CptError::InvalidGef("ifcx missing data array".into()))?;
    reconstruct_from_entities(&raw, data)
}

/// Best-effort reconstruction from raw IFC entities when there's no
/// embedded openGeoStudio block (third-party IFCX input).
fn reconstruct_from_entities(raw: &Value, data: &[Value]) -> Result<ProjectFile, CptError> {
    // Pick the IfcProject (or first project-like entity).
    let project_ent = data
        .iter()
        .find(|e| e.get("type").and_then(|t| t.as_str()) == Some("IfcProject"));
    let project = if let Some(p) = project_ent {
        ProjectInfo {
            kind: default_project_type(),
            title: p.get("Name").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string(),
            client: p.get("Description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            location: p.get("LongName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            project_number: String::new(),
            author: raw
                .get("header")
                .and_then(|h| h.get("author"))
                .and_then(|a| a.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            date: chrono::Local::now().date_naive(),
        }
    } else {
        ProjectInfo {
            kind: default_project_type(),
            title: "Untitled".to_string(),
            client: String::new(),
            location: String::new(),
            project_number: String::new(),
            author: String::new(),
            date: chrono::Local::now().date_naive(),
        }
    };

    let timestamp = raw
        .get("header")
        .and_then(|h| h.get("timeStamp"))
        .and_then(|v| v.as_str())
        .unwrap_or(&chrono::Utc::now().to_rfc3339())
        .to_string();

    Ok(ProjectFile {
        header: Header {
            schema: SCHEMA_VERSION.into(),
            originating_system: "Open Geotechniek Studio".into(),
            timestamp,
        },
        project,
        // Boreholes / strata / measurement-points round-trip would
        // require parsing property-sets; that's only justified when we
        // actually have third-party IFCX consumers. For now we leave
        // the CPTs empty on this path — the embedded payload route
        // covers the common case (our own files).
        cpts: Vec::new(),
        bores: Vec::new(),
        crs: Crs::default(),
        tekening: None,
        title_block: None,
        gis: None,
        deliverable: None,
        calculations: Vec::new(),
    })
}

/// Emit the IFC entities for one CPT: IfcBorehole + IfcLocalPlacement
/// + IfcCartesianPoint + IfcPropertySet (with the measurement vectors).
fn emit_cpt_entities(data: &mut Vec<Value>, cpt: &Cpt, crs_ref: &str) {
    let (x, y, z) = cpt
        .position
        .map(|p| (p.x_rd, p.y_rd, p.z_nap.unwrap_or(0.0)))
        .unwrap_or((0.0, 0.0, 0.0));
    let point_id = ref_id("point", &cpt.id);
    let placement_id = ref_id("placement", &cpt.id);
    let bh_id = ref_id("cpt", &cpt.id);
    let bh_guid = ifc_guid(&format!("{GUID_NAMESPACE}:cpt:{}", cpt.id));

    data.push(json!({
        "type": "IfcCartesianPoint",
        "id": point_id.trim_start_matches('#'),
        "Coordinates": [x, y, z],
    }));
    data.push(json!({
        "type": "IfcLocalPlacement",
        "id": placement_id.trim_start_matches('#'),
        "RelativePlacement": point_id,
    }));
    // IfcBorehole — IFC4x3 doesn't have a CPT-specific PredefinedType,
    // so we use the generic geotechnical investigation enum and tag the
    // ObjectType so OGS-aware readers know it's a CPT.
    data.push(json!({
        "type": "IfcBorehole",
        "id": bh_id.trim_start_matches('#'),
        "GlobalId": bh_guid,
        "Name": cpt.id,
        "Description": format!("CPT — {} measurement points", cpt.points.len()),
        "ObjectType": "ConePenetrationTest",
        "PredefinedType": "GEOTECHNICAL_DRILLING_INVESTIGATION",
        "ObjectPlacement": placement_id,
        "ProjectedCRS": crs_ref,
        "ContainedInStructure": "#site",
    }));

    // ─── Measurement vectors as IfcPropertySet/IfcPropertyTableValue ─
    let depths: Vec<Value> = cpt.points.iter().map(|p| json!(p.depth)).collect();
    let qcs: Vec<Value> = cpt
        .points
        .iter()
        .map(|p| match p.qc {
            Some(v) => json!(v),
            None => Value::Null,
        })
        .collect();
    let fss: Vec<Value> = cpt
        .points
        .iter()
        .map(|p| match p.fs {
            Some(v) => json!(v),
            None => Value::Null,
        })
        .collect();
    let rfs: Vec<Value> = cpt
        .points
        .iter()
        .map(|p| match p.rf {
            Some(v) => json!(v),
            None => Value::Null,
        })
        .collect();
    let u2s: Vec<Value> = cpt
        .points
        .iter()
        .map(|p| match p.u2 {
            Some(v) => json!(v),
            None => Value::Null,
        })
        .collect();

    let pset_id = ref_id("pset-cpt", &cpt.id);
    data.push(json!({
        "type": "IfcPropertySet",
        "id": pset_id.trim_start_matches('#'),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:cpt-pset:{}", cpt.id)),
        "Name": "OpenGeoStudio_CptMeasurements",
        "HasProperties": [
            json!({
                "type": "IfcPropertyTableValue",
                "Name": "MeasurementCurve",
                "DefiningValues": depths,
                "DefinedValues": qcs,
                "DefiningUnit": "#unit-length",
                "DefinedUnit": "#unit-pressure",
                "Expression": "qc(depth)",
            }),
            json!({
                "type": "IfcPropertyListValue",
                "Name": "FrictionSleeve_Fs",
                "ListValues": fss,
                "Unit": "#unit-pressure",
            }),
            json!({
                "type": "IfcPropertyListValue",
                "Name": "FrictionRatio_Rf",
                "ListValues": rfs,
            }),
            json!({
                "type": "IfcPropertyListValue",
                "Name": "PorePressure_U2",
                "ListValues": u2s,
                "Unit": "#unit-pressure",
            }),
        ],
    }));
    data.push(json!({
        "type": "IfcRelDefinesByProperties",
        "id": format!("rel-{}", pset_id.trim_start_matches('#')),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:cpt-rel:{}", cpt.id)),
        "RelatedObjects": [bh_id],
        "RelatingPropertyDefinition": pset_id,
    }));

    // Per-CPT metadata pset (source file, equipment, ground level).
    let meta_pset_id = ref_id("pset-cpt-meta", &cpt.id);
    let mut meta_props: Vec<Value> = Vec::new();
    meta_props.push(json_str_prop("SourceFile", &cpt.metadata.source_file));
    if let Some(eq) = &cpt.metadata.equipment {
        meta_props.push(json_str_prop("Equipment", eq));
    }
    if let Some(gl) = cpt.metadata.ground_level_nap {
        meta_props.push(json_num_prop("GroundLevelNAP", gl));
    }
    if let Some(d) = cpt.metadata.date {
        meta_props.push(json_str_prop("MeasurementDate", &d.to_string()));
    }
    if let Some(pn) = &cpt.metadata.project_name {
        meta_props.push(json_str_prop("ProjectName", pn));
    }
    if let Some(pnr) = &cpt.metadata.project_number {
        meta_props.push(json_str_prop("ProjectNumber", pnr));
    }
    data.push(json!({
        "type": "IfcPropertySet",
        "id": meta_pset_id.trim_start_matches('#'),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:cpt-meta-pset:{}", cpt.id)),
        "Name": "OpenGeoStudio_CptMetadata",
        "HasProperties": meta_props,
    }));
    data.push(json!({
        "type": "IfcRelDefinesByProperties",
        "id": format!("rel-{}", meta_pset_id.trim_start_matches('#')),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:cpt-meta-rel:{}", cpt.id)),
        "RelatedObjects": [bh_id],
        "RelatingPropertyDefinition": meta_pset_id,
    }));
}

/// Emit IFC entities for a single Bore — the bore is opaque JSON, so
/// we surface its id/coordinates if we can spot them and embed the
/// original payload in a property-set.
fn emit_bore_entities(data: &mut Vec<Value>, bore: &Value, idx: usize) {
    let bore_id = bore
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("bore-{idx}"));
    let bh_id = ref_id("bore", &bore_id);
    let bh_guid = ifc_guid(&format!("{GUID_NAMESPACE}:bore:{}", bore_id));

    data.push(json!({
        "type": "IfcBorehole",
        "id": bh_id.trim_start_matches('#'),
        "GlobalId": bh_guid,
        "Name": bore_id,
        "Description": "BHR-XML drilling investigation",
        "ObjectType": "Borehole",
        "PredefinedType": "GEOTECHNICAL_DRILLING_INVESTIGATION",
        "ContainedInStructure": "#site",
    }));
    let pset_id = ref_id("pset-bore", &bore_id);
    data.push(json!({
        "type": "IfcPropertySet",
        "id": pset_id.trim_start_matches('#'),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:bore-pset:{}", bore_id)),
        "Name": "OpenGeoStudio_BorePayload",
        "HasProperties": [json_raw_prop("Payload", bore.clone())],
    }));
    data.push(json!({
        "type": "IfcRelDefinesByProperties",
        "id": format!("rel-{}", pset_id.trim_start_matches('#')),
        "GlobalId": ifc_guid(&format!("{GUID_NAMESPACE}:bore-rel:{}", bore_id)),
        "RelatedObjects": [bh_id],
        "RelatingPropertyDefinition": pset_id,
    }));
}

/// Map our annotation-class strings to IFC4x3 `PredefinedType` values.
fn annotation_predefined_type(ifc_class: &str) -> &'static str {
    if ifc_class.ends_with("/Dimension") {
        "DIMENSION"
    } else if ifc_class.ends_with("/Line") {
        "USERDEFINED"
    } else if ifc_class.ends_with("/Overlay") {
        "USERDEFINED"
    } else {
        "USERDEFINED"
    }
}

/// Build a single-value `IfcPropertySingleValue` of type IfcLabel.
fn json_str_prop(name: &str, value: &str) -> Value {
    json!({
        "type": "IfcPropertySingleValue",
        "Name": name,
        "NominalValue": {
            "type": "IfcLabel",
            "value": value,
        }
    })
}

fn json_num_prop(name: &str, value: f64) -> Value {
    json!({
        "type": "IfcPropertySingleValue",
        "Name": name,
        "NominalValue": {
            "type": "IfcReal",
            "value": value,
        }
    })
}

fn json_bool_prop(name: &str, value: bool) -> Value {
    json!({
        "type": "IfcPropertySingleValue",
        "Name": name,
        "NominalValue": {
            "type": "IfcBoolean",
            "value": value,
        }
    })
}

/// Open-typed property that carries arbitrary JSON (geometry,
/// nested object). Not strictly IFC-spec but lets us round-trip the
/// rich payload without inventing dozens of IfcPropertyValue subtypes.
fn json_raw_prop(name: &str, value: Value) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("IfcPropertyReferenceValue".into()));
    m.insert("Name".into(), Value::String(name.into()));
    m.insert("PropertyReference".into(), value);
    Value::Object(m)
}

fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Convert decimal degrees → IFC's `[deg, min, sec, microsec]` tuple
/// (signed degrees, unsigned sub-parts; matches IfcCompoundPlaneAngleMeasure).
fn decimal_degrees_to_dms(deg: f64) -> Vec<i64> {
    let sign = if deg < 0.0 { -1.0 } else { 1.0 };
    let abs = deg.abs();
    let d = abs.trunc();
    let m_full = (abs - d) * 60.0;
    let m = m_full.trunc();
    let s_full = (m_full - m) * 60.0;
    let s = s_full.trunc();
    let micro = ((s_full - s) * 1_000_000.0).round();
    vec![
        (sign * d) as i64,
        m as i64,
        s as i64,
        micro as i64,
    ]
}

/// Generate a 22-char base64-encoded IFC GlobalId from a deterministic
/// seed (project title, cpt id, …). Reproducible saves: identical input
/// → identical GUIDs across runs.
///
/// We don't depend on the `uuid` crate (cpt-core stays slim); instead
/// we hash the seed twice with the std-lib `DefaultHasher` to produce
/// 16 bytes, then base64-encode (URL-safe, no padding) for the 22 chars.
pub(crate) fn ifc_guid(seed: &str) -> String {
    use base64::Engine;
    use std::hash::{Hash, Hasher};

    let mut h1 = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut h1);
    let v1 = h1.finish();

    let mut h2 = std::collections::hash_map::DefaultHasher::new();
    "salt".hash(&mut h2);
    seed.hash(&mut h2);
    let v2 = h2.finish();

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&v1.to_le_bytes());
    bytes[8..].copy_from_slice(&v2.to_le_bytes());

    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    // URL_SAFE_NO_PAD on 16 bytes gives 22 chars exactly. Defensive
    // truncate in case the base64 crate's behaviour changes.
    encoded.chars().take(22).collect()
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
        // `save()` writes IFCX (IFC5 alpha) now — the wire-format moved
        // from the rich `ifcgis-0.3` struct to a real IFC4X3-style
        // header + data array. Verify the output is IFCX-shaped and
        // that `load()` can sniff it back into a ProjectFile.
        let json = save(sample_project(), vec![]).unwrap();
        assert!(
            json.contains("\"schemaIdentifiers\""),
            "expected IFCX header in output, got: {json}"
        );
        assert!(
            json.contains("\"IFC4X3_ADD2\""),
            "expected IFC4X3 schema id, got: {json}"
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
        // A document that has neither IFCX shape nor the legacy
        // `ifcgis-*` schema marker must be rejected. Note: we
        // intentionally include `cpts` but NOT a `data` array so the
        // IFCX sniff doesn't match, forcing the legacy route which
        // then fails the schema-prefix check.
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
            calculations: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"schema\": \"ifcgis-0.4\""));
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

    // ─── IFCX (IFC5 alpha) wire-format tests ───────────────────────

    /// Helper: build a full ProjectFile with every section populated
    /// so round-trip tests can confirm nothing gets dropped.
    fn full_project_file() -> ProjectFile {
        ProjectFile {
            header: Header::new("Open Geotechniek Studio"),
            project: sample_project(),
            cpts: vec![sample_cpt("S01"), sample_cpt("S02")],
            bores: vec![serde_json::json!({
                "id": "BHR-01",
                "x_rd": 100_500.0,
                "y_rd": 400_500.0,
                "layers": []
            })],
            crs: Crs::default(),
            tekening: None,
            title_block: Some(TitleBlock {
                project: "Test project".into(),
                project_number: "2026-001".into(),
                drawing_number: "T-001".into(),
                scale: "1:500".into(),
                date: "2026-05-19".into(),
                drawn_by: "OGS".into(),
                ..Default::default()
            }),
            gis: Some(sample_gis()),
            deliverable: Some(sample_deliverable()),
            calculations: Vec::new(),
        }
    }

    /// (1) Empty project survives the IFCX round-trip — confirms the
    ///     header and infrastructure entities are enough on their own.
    #[test]
    fn to_ifcx_round_trips_empty_project() {
        let file = ProjectFile {
            header: Header::new("Open Geotechniek Studio"),
            project: sample_project(),
            cpts: Vec::new(),
            bores: Vec::new(),
            crs: Crs::default(),
            tekening: None,
            title_block: None,
            gis: None,
            deliverable: None,
            calculations: Vec::new(),
        };
        let ifcx = to_ifcx_json(&file).expect("to_ifcx_json should succeed");
        // The output is IFCX-shaped (not legacy).
        let v: serde_json::Value = serde_json::from_str(&ifcx).unwrap();
        assert!(v.get("header").is_some(), "missing header");
        assert!(v.get("data").is_some(), "missing data array");
        // Round-trip back.
        let back = from_ifcx_json(&ifcx).expect("from_ifcx_json should succeed");
        assert_eq!(back.project.title, file.project.title);
        assert_eq!(back.project.client, file.project.client);
        assert_eq!(back.project.location, file.project.location);
        assert_eq!(back.project.project_number, file.project.project_number);
        assert_eq!(back.project.author, file.project.author);
        assert_eq!(back.project.date, file.project.date);
        assert_eq!(back.cpts.len(), 0);
        assert_eq!(back.bores.len(), 0);
        assert!(back.tekening.is_none());
        assert!(back.title_block.is_none());
        assert!(back.gis.is_none());
        assert!(back.deliverable.is_none());
        assert_eq!(back.crs.epsg, 28992);
    }

    /// (2) Output contains an `IfcProject` entity with the right Name —
    ///     proves the IFC mapping actually emitted the project entity.
    #[test]
    fn to_ifcx_includes_ifc_project_entity() {
        let file = full_project_file();
        let ifcx = to_ifcx_json(&file).unwrap();
        let v: serde_json::Value = serde_json::from_str(&ifcx).unwrap();
        let data = v.get("data").and_then(|d| d.as_array()).expect("data array");
        let project_ent = data
            .iter()
            .find(|e| e.get("type").and_then(|t| t.as_str()) == Some("IfcProject"))
            .expect("IfcProject entity should exist");
        assert_eq!(
            project_ent.get("Name").and_then(|n| n.as_str()),
            Some("Test project"),
        );
        // GlobalId is the 22-char IFC GUID.
        let gid = project_ent
            .get("GlobalId")
            .and_then(|g| g.as_str())
            .expect("GlobalId present");
        assert_eq!(gid.len(), 22, "IFC GlobalId must be 22 chars, got {}: {}", gid.len(), gid);
        // Deterministic — same seed → same GUID.
        assert_eq!(
            gid,
            ifc_guid(&format!("{GUID_NAMESPACE}:project:{}", file.project.title)),
        );
        // RepresentationContexts cross-references look right.
        let ctxs = project_ent
            .get("RepresentationContexts")
            .and_then(|c| c.as_array())
            .expect("RepresentationContexts");
        assert!(ctxs.iter().any(|c| c.as_str() == Some("#ctx-3d")));
        assert!(ctxs.iter().any(|c| c.as_str() == Some("#ctx-2d")));
    }

    /// (3) Legacy `ifcgis-0.3` files still load — confirms the
    ///     load() sniff routes them to the legacy parser.
    #[test]
    fn from_ifcx_loads_legacy_0_3() {
        // Hand-rolled 0.3 JSON exactly as it would appear on disk
        // before the IFCX migration.
        let legacy_0_3 = r#"{
            "header": {
                "schema": "ifcgis-0.3",
                "originating_system": "Open Geotechniek Studio",
                "timestamp": "2026-04-01T08:00:00+00:00"
            },
            "project": {
                "type": "OpenGeoProject",
                "title": "Legacy 0.3 project",
                "client": "ACME bv",
                "location": "Den Haag",
                "project_number": "2026-099",
                "author": "OGS",
                "date": "2026-04-01"
            },
            "cpts": [],
            "crs": { "epsg": 28992, "name": "Amersfoort / RD New" },
            "gis": {
                "epsg": 28992,
                "name": "Amersfoort / RD New",
                "layers": []
            }
        }"#;
        let back = load(legacy_0_3)
            .expect("legacy ifcgis-0.3 must still load on the new IFCX-aware loader");
        assert_eq!(back.header.schema, "ifcgis-0.3");
        assert_eq!(back.project.title, "Legacy 0.3 project");
        assert_eq!(back.project.location, "Den Haag");
        assert_eq!(back.crs.epsg, 28992);
        assert!(back.gis.is_some(), "gis section should be parsed");
        assert!(back.deliverable.is_none());
    }

    /// (4) Output has a `data` array containing multiple entities
    ///     (project, owner-history, units, site, contexts, …).
    #[test]
    fn to_ifcx_includes_data_array() {
        let file = full_project_file();
        let ifcx = to_ifcx_json(&file).unwrap();
        let v: serde_json::Value = serde_json::from_str(&ifcx).unwrap();
        let data = v.get("data").and_then(|d| d.as_array()).expect("data array");
        // Spot-check that core infrastructure entities are present.
        let types: std::collections::BTreeSet<&str> = data
            .iter()
            .filter_map(|e| e.get("type").and_then(|t| t.as_str()))
            .collect();
        for required in [
            "IfcProject",
            "IfcSite",
            "IfcOwnerHistory",
            "IfcUnitAssignment",
            "IfcSIUnit",
            "IfcGeometricRepresentationContext",
            "IfcProjectedCRS",
            "IfcMapConversion",
            "IfcPostalAddress",
            "IfcBorehole",
            "IfcPropertySet",
        ] {
            assert!(
                types.contains(required),
                "expected IFC entity '{required}' in data array; got types: {types:?}",
            );
        }
        // The data array should have well over 10 entries with all
        // the CPTs/bores/layers/sheet/annotations emitted.
        assert!(
            data.len() >= 10,
            "expected at least 10 entities in data, got {}",
            data.len(),
        );
        // Round-trip via load() picks the IFCX route and recovers
        // everything (CPTs, bores, GIS, deliverable, title-block).
        let back = load(&ifcx).expect("load() should accept IFCX output");
        assert_eq!(back.cpts.len(), 2);
        assert_eq!(back.cpts[0].id, "S01");
        assert_eq!(back.bores.len(), 1);
        assert!(back.gis.is_some());
        assert!(back.deliverable.is_some());
        assert!(back.title_block.is_some());
    }

    /// Sanity-check on the GUID generator: deterministic + 22 chars.
    #[test]
    fn ifc_guid_is_22_chars_and_deterministic() {
        let a = ifc_guid("ogs:project:Test");
        let b = ifc_guid("ogs:project:Test");
        let c = ifc_guid("ogs:project:Other");
        assert_eq!(a.len(), 22);
        assert_eq!(a, b, "same seed should give same GUID");
        assert_ne!(a, c, "different seeds should give different GUIDs");
        // URL-safe base64 alphabet only (no padding chars).
        for ch in a.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "unexpected char '{ch}' in GUID {a}",
            );
        }
    }
}
