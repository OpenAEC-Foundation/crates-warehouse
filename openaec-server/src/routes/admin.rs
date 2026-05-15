use axum::extract::Path as AxumPath;
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::helpers::tenant_config;

fn count_files_in_dir(dir: &std::path::Path, extensions: &[&str]) -> usize {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| extensions.iter().any(|x| ext == *x))
        })
        .count()
}

pub async fn list_tenants() -> Json<Value> {
    let tc = tenant_config();
    let mut tenants = Vec::new();

    if let Some(root) = tc.tenants_root() {
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
            dirs.sort_by_key(|e| e.file_name());

            for entry in dirs {
                let path = entry.path();
                let name = entry.file_name().to_str().unwrap_or("").to_string();

                tenants.push(json!({
                    "name": name,
                    "has_brand": path.join("brand.yaml").is_file(),
                    "template_count": count_files_in_dir(&path.join("templates"), &["yaml"]),
                    "page_type_count": count_files_in_dir(&path.join("page_types"), &["yaml"]),
                    "module_count": count_files_in_dir(&path.join("modules"), &["yaml"]),
                    "stationery_count": count_files_in_dir(&path.join("stationery"), &["pdf", "png", "jpg"]),
                    "logo_count": count_files_in_dir(&path.join("logos"), &["svg", "png", "jpg"]),
                    "font_count": count_files_in_dir(&path.join("fonts"), &["ttf", "otf", "woff", "woff2"]),
                }));
            }
        }
    }

    Json(json!({ "tenants": tenants }))
}

pub async fn list_tenant_templates(
    AxumPath(tenant): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let tc = tenant_config();
    let root = tc
        .tenants_root()
        .ok_or_else(|| AppError::NotFound("No tenants root".to_string()))?;

    let templates_dir = root.join(&tenant).join("templates");
    if !templates_dir.is_dir() {
        return Ok(Json(json!({ "templates": [] })));
    }

    let templates: Vec<Value> = std::fs::read_dir(&templates_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml"))
        .map(|e| {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            json!({
                "filename": e.file_name().to_str().unwrap_or(""),
                "size": size,
            })
        })
        .collect();

    Ok(Json(json!({ "templates": templates })))
}

pub async fn get_tenant_brand(
    AxumPath(tenant): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let tc = tenant_config();
    let root = tc
        .tenants_root()
        .ok_or_else(|| AppError::NotFound("No tenants root".to_string()))?;

    let brand_path = root.join(&tenant).join("brand.yaml");
    if !brand_path.is_file() {
        return Ok(Json(json!({
            "exists": false,
            "parsed": null,
            "raw": "",
        })));
    }

    let raw = std::fs::read_to_string(&brand_path)?;

    let parsed: Option<Value> = serde_yaml::from_str::<serde_yaml::Value>(&raw)
        .ok()
        .and_then(|v| serde_json::to_value(v).ok());

    Ok(Json(json!({
        "exists": true,
        "parsed": parsed,
        "raw": raw,
    })))
}
