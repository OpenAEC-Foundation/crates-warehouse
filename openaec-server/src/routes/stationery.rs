use axum::Json;
use serde_json::{json, Value};

use crate::helpers::tenant_config;

pub async fn list_stationery() -> Json<Value> {
    let tc = tenant_config();
    let mut result = serde_json::Map::new();

    if let Some(root) = tc.tenants_root() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let tenant_name = entry.file_name().to_str().unwrap_or("").to_string();
                let stat_dir = path.join("stationery");
                if !stat_dir.is_dir() {
                    continue;
                }
                let files: Vec<String> = std::fs::read_dir(&stat_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .is_some_and(|ext| ext == "pdf" || ext == "png" || ext == "jpg")
                    })
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .collect();

                result.insert(
                    tenant_name,
                    json!({
                        "complete": !files.is_empty(),
                        "files": files,
                        "missing": [],
                    }),
                );
            }
        }
    }

    Json(json!({ "brands": result }))
}
