//! Kadaster client — PDOK API integration for cadastral map material.
//!
//! Port of `data/kadaster.py`. Uses polynomial approximation for
//! WGS84 ↔ Rijksdriehoek coordinate conversion (no `proj` dependency).
//! PDOK WMS services for map images.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// PDOK WMS service URLs.
pub const WMS_KADASTER: &str = "https://service.pdok.nl/kadaster/kadastralekaart/wms/v5_0";
pub const WMS_BGT: &str = "https://service.pdok.nl/lv/bgt/wms/v1_0";
pub const WMS_LUCHTFOTO: &str = "https://service.pdok.nl/hwh/luchtfotorgb/wms/v1_0";
pub const WMS_BAG: &str = "https://service.pdok.nl/lv/bag/wms/v2_0";

// ── Coordinate conversion (RDNAPTRANS polynomial) ──────────────────────

// Reference points for the polynomial approximation
const X0: f64 = 155_000.0; // RD x reference
const Y0: f64 = 463_000.0; // RD y reference
const PHI0: f64 = 52.156_160_556; // WGS84 lat reference (Amersfoort)
const LAM0: f64 = 5.387_638_889; // WGS84 lon reference (Amersfoort)

/// Convert WGS84 coordinates to Rijksdriehoek (RD/EPSG:28992).
///
/// Uses the RDNAPTRANS polynomial approximation.
/// Accuracy: ~1m (sufficient for map display, not for geodetic work).
///
/// # Arguments
/// * `lat` — Latitude in WGS84 degrees.
/// * `lon` — Longitude in WGS84 degrees.
///
/// # Returns
/// Tuple `(x, y)` in RD coordinates (meters).
pub fn wgs84_to_rd(lat: f64, lon: f64) -> (f64, f64) {
    let d_phi = 0.36 * (lat - PHI0);
    let d_lam = 0.36 * (lon - LAM0);

    let x = X0
        + 190_094.945 * d_lam
        + (-11_832.228) * d_phi * d_lam
        + (-114.221) * d_phi.powi(2) * d_lam
        + (-32.391) * d_lam.powi(3)
        + (-0.705) * d_phi
        + (-2.340) * d_phi.powi(3) * d_lam
        + (-0.608) * d_phi * d_lam.powi(3)
        + (-0.008) * d_lam.powi(2)
        + 0.148 * d_phi.powi(2) * d_lam.powi(3);

    let y = Y0
        + 309_056.544 * d_phi
        + 3_638.893 * d_lam.powi(2)
        + 73.077 * d_phi.powi(2)
        + (-157.984) * d_phi * d_lam.powi(2)
        + 59.788 * d_phi.powi(3)
        + 0.433 * d_lam
        + (-6.439) * d_phi.powi(2) * d_lam.powi(2)
        + (-0.032) * d_phi * d_lam
        + 0.092 * d_lam.powi(4)
        + (-0.054) * d_phi * d_lam.powi(4);

    (x, y)
}

/// Convert Rijksdriehoek (RD/EPSG:28992) to WGS84 coordinates.
///
/// Inverse polynomial approximation.
///
/// # Arguments
/// * `x` — RD x-coordinate (meters).
/// * `y` — RD y-coordinate (meters).
///
/// # Returns
/// Tuple `(lat, lon)` in WGS84 degrees.
pub fn rd_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    let d_x = (x - X0) * 1e-5;
    let d_y = (y - Y0) * 1e-5;

    let lat = PHI0
        + (3_235.653_99 * d_y
            + (-32.587_01) * d_x.powi(2)
            + (-0.247_50) * d_y.powi(2)
            + (-0.849_78) * d_x.powi(2) * d_y
            + (-0.065_5) * d_y.powi(3)
            + (-0.018_35) * d_x.powi(2) * d_y.powi(2)
            + (-0.004_71) * d_x.powi(4)
            + 0.005_2 * d_y.powi(4))
            / 3600.0;

    let lon = LAM0
        + (5_260.528_16 * d_x
            + 105.947_84 * d_x * d_y
            + 2.459_3 * d_x * d_y.powi(2)
            + (-0.819_39) * d_x.powi(3)
            + 0.036_59 * d_x * d_y.powi(3)
            + (-0.004_44) * d_x.powi(3) * d_y
            + (-0.003_2) * d_x.powi(5))
            / 3600.0;

    (lat, lon)
}

// ── Kadaster WMS client ────────────────────────────────────────────────

/// Error type for Kadaster operations.
#[derive(Debug, thiserror::Error)]
pub enum KadasterError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unknown WMS service: {0}")]
    UnknownService(String),
}

/// Client for PDOK WMS services.
///
/// Retrieves cadastral map material via the free PDOK API.
/// No API key required.
///
/// Available services:
/// - `kadaster` — Cadastral map (parcel boundaries, numbers)
/// - `bgt` — Base Registration Large-Scale Topography
/// - `luchtfoto` — Aerial photography
/// - `bag` — Building and Address Registration
pub struct KadasterClient {
    #[allow(dead_code)]
    cache_dir: Option<PathBuf>,
    client: reqwest::blocking::Client,
}

impl KadasterClient {
    /// Create a new client.
    ///
    /// # Arguments
    /// * `cache_dir` — Optional directory for cached map images.
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        if let Some(ref dir) = cache_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        Self {
            cache_dir,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Get the WMS URL for a service name.
    pub fn wms_url(service: &str) -> Result<&'static str, KadasterError> {
        match service {
            "kadaster" => Ok(WMS_KADASTER),
            "bgt" => Ok(WMS_BGT),
            "luchtfoto" => Ok(WMS_LUCHTFOTO),
            "bag" => Ok(WMS_BAG),
            _ => Err(KadasterError::UnknownService(service.to_string())),
        }
    }

    /// Retrieve a map image via WMS GetMap.
    ///
    /// # Arguments
    /// * `lat` — Latitude (WGS84).
    /// * `lon` — Longitude (WGS84).
    /// * `radius_m` — Radius around the point in meters (default: 100).
    /// * `width_px` — Image width in pixels (default: 800).
    /// * `height_px` — Image height in pixels (default: 600).
    /// * `service` — WMS service name (default: "kadaster").
    /// * `layers` — Comma-separated layer names.
    /// * `image_format` — Output format (default: "image/png").
    #[allow(clippy::too_many_arguments)]
    pub fn get_map(
        &self,
        lat: f64,
        lon: f64,
        radius_m: f64,
        width_px: u32,
        height_px: u32,
        service: &str,
        layers: &str,
        image_format: &str,
    ) -> Result<Vec<u8>, KadasterError> {
        let (x, y) = wgs84_to_rd(lat, lon);
        let bbox = format!(
            "{},{},{},{}",
            x - radius_m,
            y - radius_m,
            x + radius_m,
            y + radius_m
        );

        let url = Self::wms_url(service)?;

        let params: HashMap<&str, String> = HashMap::from([
            ("service", "WMS".to_string()),
            ("version", "1.3.0".to_string()),
            ("request", "GetMap".to_string()),
            ("layers", layers.to_string()),
            ("crs", "EPSG:28992".to_string()),
            ("bbox", bbox),
            ("width", width_px.to_string()),
            ("height", height_px.to_string()),
            ("format", image_format.to_string()),
            ("transparent", "true".to_string()),
        ]);

        let response = self.client.get(url).query(&params).send()?;
        let bytes = response.error_for_status()?.bytes()?;

        Ok(bytes.to_vec())
    }

    /// Retrieve a map with default settings.
    pub fn get_map_default(&self, lat: f64, lon: f64) -> Result<Vec<u8>, KadasterError> {
        self.get_map(
            lat,
            lon,
            100.0,
            800,
            600,
            "kadaster",
            "Perceel,OpenbareRuimteNaam",
            "image/png",
        )
    }

    /// Retrieve a map and save to file.
    #[allow(clippy::too_many_arguments)]
    pub fn save_map(
        &self,
        lat: f64,
        lon: f64,
        output_path: &Path,
        radius_m: f64,
        width_px: u32,
        height_px: u32,
        service: &str,
        layers: &str,
    ) -> Result<PathBuf, KadasterError> {
        let image_data = self.get_map(
            lat,
            lon,
            radius_m,
            width_px,
            height_px,
            service,
            layers,
            "image/png",
        )?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(output_path, &image_data)?;
        Ok(output_path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Amersfoort (reference point) should map to (155000, 463000)
    #[test]
    fn test_wgs84_to_rd_amersfoort() {
        let (x, y) = wgs84_to_rd(PHI0, LAM0);
        assert!((x - X0).abs() < 1.0, "x={x}, expected ~{X0}");
        assert!((y - Y0).abs() < 1.0, "y={y}, expected ~{Y0}");
    }

    #[test]
    fn test_rd_to_wgs84_amersfoort() {
        let (lat, lon) = rd_to_wgs84(X0, Y0);
        assert!(
            (lat - PHI0).abs() < 0.0001,
            "lat={lat}, expected ~{PHI0}"
        );
        assert!(
            (lon - LAM0).abs() < 0.0001,
            "lon={lon}, expected ~{LAM0}"
        );
    }

    // Roundtrip test: WGS84 → RD → WGS84
    #[test]
    fn test_roundtrip_amsterdam() {
        let lat = 52.3676;
        let lon = 4.9041;
        let (x, y) = wgs84_to_rd(lat, lon);
        let (lat2, lon2) = rd_to_wgs84(x, y);
        assert!(
            (lat - lat2).abs() < 0.001,
            "lat roundtrip: {lat} -> {lat2}"
        );
        assert!(
            (lon - lon2).abs() < 0.001,
            "lon roundtrip: {lon} -> {lon2}"
        );
    }

    #[test]
    fn test_roundtrip_rotterdam() {
        let lat = 51.9225;
        let lon = 4.4792;
        let (x, y) = wgs84_to_rd(lat, lon);
        let (lat2, lon2) = rd_to_wgs84(x, y);
        assert!(
            (lat - lat2).abs() < 0.001,
            "lat roundtrip: {lat} -> {lat2}"
        );
        assert!(
            (lon - lon2).abs() < 0.001,
            "lon roundtrip: {lon} -> {lon2}"
        );
    }

    #[test]
    fn test_wms_url_known() {
        assert_eq!(KadasterClient::wms_url("kadaster").unwrap(), WMS_KADASTER);
        assert_eq!(KadasterClient::wms_url("bgt").unwrap(), WMS_BGT);
        assert_eq!(
            KadasterClient::wms_url("luchtfoto").unwrap(),
            WMS_LUCHTFOTO
        );
        assert_eq!(KadasterClient::wms_url("bag").unwrap(), WMS_BAG);
    }

    #[test]
    fn test_wms_url_unknown() {
        assert!(KadasterClient::wms_url("nonexistent").is_err());
    }

    // Known coordinate: Dam Square, Amsterdam
    #[test]
    fn test_dam_square() {
        let lat = 52.3731;
        let lon = 4.8932;
        let (x, y) = wgs84_to_rd(lat, lon);
        // Dam Square ≈ RD (121000, 487000) — rough check
        assert!(
            (x - 121_000.0).abs() < 500.0,
            "Dam x={x}, expected ~121000"
        );
        assert!(
            (y - 487_000.0).abs() < 500.0,
            "Dam y={y}, expected ~487000"
        );
    }
}
