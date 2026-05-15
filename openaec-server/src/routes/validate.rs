use axum::Json;
use serde_json::{json, Value};

use crate::error::AppError;

pub async fn validate(Json(payload): Json<Value>) -> Result<Json<Value>, AppError> {
    let json_str = serde_json::to_string(&payload)?;

    match openaec_core::ReportData::from_json(&json_str) {
        Ok(report) => Ok(Json(json!({
            "valid": true,
            "sections": report.sections.len(),
            "template": report.template
        }))),
        Err(e) => Ok(Json(json!({
            "valid": false,
            "error": e.to_string()
        }))),
    }
}
