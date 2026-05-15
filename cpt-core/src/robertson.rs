//! Robertson 1990 SBT classification (simplified, qc + Rf only).
//!
//! Direct port of the Dutch geotechnical practice approximation
//! used in the previous JS implementation. Returns one of 9 zones.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Zone {
    pub number: u8,
    pub name: &'static str,
    pub color: &'static str,
}

const ZONES: [Zone; 9] = [
    Zone { number: 1, name: "Gevoelig fijnkorrelig",  color: "#00BCD4" },
    Zone { number: 2, name: "Organisch / veen",        color: "#795548" },
    Zone { number: 3, name: "Klei",                    color: "#4CAF50" },
    Zone { number: 4, name: "Silt mengsels",           color: "#8BC34A" },
    Zone { number: 5, name: "Zand mengsels",           color: "#FFC107" },
    Zone { number: 6, name: "Zand",                    color: "#FF9800" },
    Zone { number: 7, name: "Grof zand / grind",       color: "#FF5722" },
    Zone { number: 8, name: "Zeer vast zand/klei",     color: "#F44336" },
    Zone { number: 9, name: "Zeer vast fijnkorrelig",  color: "#9C27B0" },
];

pub fn zones() -> &'static [Zone] {
    &ZONES
}

/// Classify a measurement point by cone resistance and friction ratio.
/// Returns `None` for invalid inputs (qc <= 0 or rf < 0).
pub fn classify(qc: f64, rf: f64) -> Option<Zone> {
    if qc <= 0.0 || rf < 0.0 {
        return None;
    }
    if qc > 25.0 {
        if rf < 1.0 { return Some(ZONES[6]); }   // Zone 7
        return Some(ZONES[7]);                    // Zone 8
    }
    if qc > 10.0 {
        if rf < 0.5 { return Some(ZONES[6]); }   // Zone 7
        if rf < 1.5 { return Some(ZONES[5]); }   // Zone 6
        if rf < 3.0 { return Some(ZONES[4]); }   // Zone 5
        return Some(ZONES[7]);                    // Zone 8
    }
    if qc > 5.0 {
        if rf < 1.0 { return Some(ZONES[5]); }   // Zone 6
        if rf < 2.0 { return Some(ZONES[4]); }   // Zone 5
        if rf < 4.0 { return Some(ZONES[3]); }   // Zone 4
        if rf < 6.0 { return Some(ZONES[2]); }   // Zone 3
        return Some(ZONES[8]);                    // Zone 9
    }
    if qc > 2.0 {
        if rf < 1.0 { return Some(ZONES[4]); }   // Zone 5
        if rf < 2.5 { return Some(ZONES[3]); }   // Zone 4
        if rf < 5.0 { return Some(ZONES[2]); }   // Zone 3
        return Some(ZONES[1]);                    // Zone 2
    }
    if qc > 0.5 {
        if rf < 1.0 { return Some(ZONES[3]); }   // Zone 4
        if rf < 3.0 { return Some(ZONES[2]); }   // Zone 3
        return Some(ZONES[1]);                    // Zone 2
    }
    // qc 0..0.5
    if rf < 2.0 { return Some(ZONES[2]); }       // Zone 3
    if rf < 5.0 { return Some(ZONES[1]); }       // Zone 2
    Some(ZONES[0])                                // Zone 1
}
