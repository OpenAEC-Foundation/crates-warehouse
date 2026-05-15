use axum::Json;
use serde_json::{json, Value};

use crate::helpers::tenant_config;

pub async fn list_brands() -> Json<Value> {
    let tc = tenant_config();
    let mut brands = Vec::new();

    if let Some(td) = tc.tenant_dir() {
        let brand_path = td.join("brand.yaml");
        if brand_path.is_file() {
            let name = td
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default");
            brands.push(json!({
                "name": name.replace('_', " "),
                "slug": name,
            }));
        }
    }

    if let Some(root) = tc.tenants_root() {
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut tenant_names: Vec<String> = entries
                .flatten()
                .filter(|e| e.path().is_dir() && e.path().join("brand.yaml").is_file())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect();
            tenant_names.sort();

            for name in tenant_names {
                if brands.iter().any(|b| b["slug"] == name) {
                    continue;
                }
                brands.push(json!({
                    "name": name.replace('_', " "),
                    "slug": name,
                }));
            }
        }
    }

    if brands.is_empty() {
        brands.push(json!({ "name": "default", "slug": "default" }));
    }

    Json(json!({ "brands": brands }))
}
