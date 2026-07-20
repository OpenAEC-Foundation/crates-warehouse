//! WASM-bindings: dezelfde importers als de CLI, maar dan in de browser.
//! Gebouwd met `wasm-pack build --target web`.

use wasm_bindgen::prelude::*;

/// Converteert één KLIC-leveringszip (bytes) naar Baken-profiel GeoJSON.
/// Geeft de GeoJSON-string terug, of gooit een JS-error met een nette melding.
#[wasm_bindgen]
pub fn convert_klic_zip(bytes: &[u8], bestandsnaam: &str, project: &str) -> Result<String, JsError> {
    let levering = klic2geo::convert_zip_bytes(bytes, bestandsnaam)
        .map_err(|e| JsError::new(&e))?;
    let fc = klic2geo::feature_collection(
        &format!("klic-{project}"),
        project,
        &[bestandsnaam.to_string()],
        &[levering],
    );
    serde_json::to_string(&fc).map_err(|e| JsError::new(&e.to_string()))
}

/// Converteert een plankaart (DWG of DXF, bytes) naar Baken-profiel GeoJSON
/// (laagtype `ontwerp`): geometrie + PMKL-thema uit de laagnaam.
#[wasm_bindgen]
pub fn convert_cad(bytes: &[u8], bestandsnaam: &str, project: &str) -> Result<String, JsError> {
    let fc = plan2geo::convert_cad_bytes(bytes, bestandsnaam, project)
        .map_err(|e| JsError::new(&e))?;
    serde_json::to_string(&fc).map_err(|e| JsError::new(&e.to_string()))
}

/// Baken-ontwerplaag (GeoJSON) → DXF-bytes (ASCII CAD-interchange, opent in
/// elk CAD-pakket) via RUST-DWG.
#[wasm_bindgen]
pub fn export_dxf(geojson: &str) -> Result<Vec<u8>, JsError> {
    plan2geo::geojson_to_dxf(geojson).map_err(|e| JsError::new(&e))
}

/// Baken-ontwerplaag (GeoJSON) → native DWG-bytes via RUST-DWG.
#[wasm_bindgen]
pub fn export_dwg(geojson: &str) -> Result<Vec<u8>, JsError> {
    plan2geo::geojson_to_dwg(geojson).map_err(|e| JsError::new(&e))
}

/// Versie-info voor de UI.
#[wasm_bindgen]
pub fn importer_versie() -> String {
    format!("baken-import {} · {}", env!("CARGO_PKG_VERSION"), baken_schema())
}

fn baken_schema() -> &'static str {
    "baken-geo/1"
}
