//! Layer detection: groups consecutive measurement points with the same
//! Robertson zone into layers. Layers thinner than `MIN_LAYER_THICKNESS`
//! are merged into their surroundings.

use serde::{Deserialize, Serialize};

use crate::domain::Cpt;
use crate::robertson::{classify, Zone};

const MIN_LAYER_THICKNESS: f64 = 0.10; // 10 cm

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub depth_top: f64,
    pub depth_bottom: f64,
    pub zone_number: u8,
    pub zone_name: &'static str,
    pub zone_color: &'static str,
}

impl Layer {
    pub fn thickness(&self) -> f64 { self.depth_bottom - self.depth_top }
    fn from_zone(top: f64, bottom: f64, z: Zone) -> Self {
        Self { depth_top: top, depth_bottom: bottom, zone_number: z.number,
               zone_name: z.name, zone_color: z.color }
    }
}

pub fn detect_layers(cpt: &Cpt) -> Vec<Layer> {
    if cpt.points.is_empty() { return Vec::new(); }

    // 1. Classify every point; skip ones we can't classify.
    let classified: Vec<(f64, Zone)> = cpt.points.iter()
        .filter_map(|p| {
            let qc = p.qc?; let rf = p.rf?;
            classify(qc, rf).map(|z| (p.depth, z))
        })
        .collect();
    if classified.is_empty() { return Vec::new(); }

    // 2. Group consecutive same-zone points into raw layers.
    let mut raw: Vec<Layer> = Vec::new();
    let mut start = classified[0].0;
    let mut current_zone = classified[0].1;
    for window in classified.windows(2) {
        let (depth, zone) = window[1];
        if zone.number != current_zone.number {
            raw.push(Layer::from_zone(start, window[0].0, current_zone));
            start = depth;
            current_zone = zone;
        }
    }
    let last_depth = classified.last().unwrap().0;
    raw.push(Layer::from_zone(start, last_depth, current_zone));

    // 3. Merge layers thinner than threshold into the previous one,
    //    and coalesce neighbours that end up sharing the same zone
    //    (a thin layer absorbed into its predecessor must not leave the
    //    next same-zone layer dangling as a separate entry).
    let mut merged: Vec<Layer> = Vec::new();
    for layer in raw {
        if let Some(last) = merged.last_mut() {
            if layer.thickness() < MIN_LAYER_THICKNESS {
                last.depth_bottom = layer.depth_bottom;
                continue;
            }
            if last.zone_number == layer.zone_number {
                last.depth_bottom = layer.depth_bottom;
                continue;
            }
        }
        merged.push(layer);
    }
    merged
}
