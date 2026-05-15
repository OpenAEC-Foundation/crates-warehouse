use axum::extract::Path as AxumPath;
use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::helpers::template_loader;

pub async fn list_templates() -> Json<Value> {
    let loader = template_loader();
    let templates: Vec<Value> = loader
        .list_templates()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "report_type": t.report_type,
            })
        })
        .collect();

    Json(json!({ "templates": templates }))
}

pub async fn get_scaffold(
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let loader = template_loader();
    match loader.to_scaffold(&name) {
        Ok(scaffold) => Ok(Json(scaffold)),
        Err(e) => Err(AppError::NotFound(e.to_string())),
    }
}
