//! RD (Rijksdriehoek New / Amersfoort) <-> WGS84 conversion.
//!
//! Uses the standardized polynomial approximation published by Kadaster
//! (sub-meter accuracy across the Netherlands). No external dependency,
//! no HTTP calls. Sources:
//! - https://www.kadaster.nl/zakelijk/diensten/handleiding-rdnaptrans
//! - Schreutelkamp & Strang van Hees, "Benaderingsformules voor de
//!   transformatie tussen RD en WGS84-coordinaten" (2001).

const X0: f64 = 155_000.0;
const Y0: f64 = 463_000.0;
// Amersfoort RD origin expressed in WGS84 (degrees). The Schreutelkamp 2001
// polynomial is calibrated against these WGS84 reference values, so applying
// the deltas directly yields WGS84 output (no separate Bessel->WGS84 shift).
const PHI0: f64 = 52.155_174_40;    // Amersfoort lat WGS84 (deg)
const LAM0: f64 = 5.387_206_21;     // Amersfoort lon WGS84 (deg)

/// Convert RD (x, y) in meters to WGS84 (latitude, longitude) in degrees.
pub fn rd_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    let dx = (x - X0) * 1.0e-5;
    let dy = (y - Y0) * 1.0e-5;

    let kp = [
        (0, 1,  3235.65389),
        (2, 0,  -32.58297),
        (0, 2,  -0.24750),
        (2, 1,  -0.84978),
        (0, 3,  -0.06550),
        (2, 2,  -0.01709),
        (1, 0,  -0.00738),
        (4, 0,   0.00530),
        (2, 3,  -0.00039),
        (4, 1,   0.00033),
        (1, 1,  -0.00012),
    ];
    let lp = [
        (1, 0,  5260.52916),
        (1, 1,  105.94684),
        (1, 2,   2.45656),
        (3, 0,  -0.81885),
        (1, 3,   0.05594),
        (3, 1,  -0.05607),
        (0, 1,   0.01199),
        (3, 2,  -0.00256),
        (1, 4,   0.00128),
        (0, 2,   0.00022),
        (2, 0,  -0.00022),
        (5, 0,   0.00026),
    ];

    let mut dphi = 0.0;
    for &(p, q, k) in &kp {
        dphi += k * dx.powi(p) * dy.powi(q);
    }
    let mut dlam = 0.0;
    for &(p, q, l) in &lp {
        dlam += l * dx.powi(p) * dy.powi(q);
    }
    let phi = PHI0 + dphi / 3600.0;
    let lam = LAM0 + dlam / 3600.0;
    (phi, lam)
}

/// Convert WGS84 (latitude, longitude) in degrees to RD (x, y) in meters.
pub fn wgs84_to_rd(lat: f64, lon: f64) -> (f64, f64) {
    let dphi = 0.36 * (lat - PHI0);
    let dlam = 0.36 * (lon - LAM0);

    let rp = [
        (0, 1,  190_094.945),
        (1, 1,  -11_832.228),
        (2, 1,    -114.221),
        (0, 3,     -32.391),
        (1, 0,      -0.705),
        (3, 1,      -2.340),
        (1, 3,      -0.608),
        (0, 2,      -0.008),
        (2, 3,       0.148),
    ];
    let sp = [
        (1, 0,  309_056.544),
        (0, 2,    3_638.893),
        (2, 0,      73.077),
        (1, 2,    -157.984),
        (3, 0,      59.788),
        (0, 1,       0.433),
        (2, 2,      -6.439),
        (1, 1,      -0.032),
        (0, 4,       0.092),
        (1, 4,      -0.054),
    ];

    let mut x = X0;
    for &(p, q, r) in &rp {
        x += r * dphi.powi(p) * dlam.powi(q);
    }
    let mut y = Y0;
    for &(p, q, s) in &sp {
        y += s * dphi.powi(p) * dlam.powi(q);
    }
    (x, y)
}
