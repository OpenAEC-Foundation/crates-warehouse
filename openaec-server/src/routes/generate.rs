use axum::{http::header, response::IntoResponse, Json};
use serde_json::Value;

use crate::error::AppError;
use crate::helpers::tenant_config;

/// Generate PDF via legacy V1/V2 engine (openaec-core).
pub async fn generate(
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let json_str = serde_json::to_string(&payload)?;

    let pdf_bytes = tokio::task::spawn_blocking(move || {
        let report_data = openaec_core::ReportData::from_json(&json_str)
            .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {}", e)))?;

        let temp_path = std::path::Path::new("temp_output.pdf");
        openaec_core::engine::generate_pdf(&report_data, temp_path)
            .map_err(|e| AppError::Internal(format!("Generation failed: {}", e)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "inline; filename=\"rapport.pdf\"",
            ),
        ],
        pdf_bytes,
    ))
}

/// Generate PDF via V3 TemplateEngine (openaec-engine).
pub async fn generate_template(
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let template_name = payload
        .get("template")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing 'template' field".to_string()))?
        .to_string();

    let tenant = payload
        .get("tenant")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            payload
                .get("brand")
                .and_then(|v| v.as_str())
                .unwrap_or("3bm_cooperatie")
        })
        .to_string();

    let tc = tenant_config();
    let tenants_root = tc.tenants_root().map(|p| p.to_path_buf());
    let tenant_dir = tc.tenant_dir().map(|p| p.to_path_buf());

    let filename = format!("{}_{}.pdf", tenant, template_name);
    let data = payload;

    let pdf_bytes = tokio::task::spawn_blocking(move || {
        let config = openaec_engine::EngineConfig {
            tenant_dir,
            tenants_root,
        };

        let engine = openaec_engine::Engine::new(config);
        engine
            .generate(&template_name, &tenant, &data)
            .map_err(|e| AppError::Internal(format!("Template generation failed: {}", e)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    Ok((
        [(header::CONTENT_TYPE, "application/pdf")],
        [
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", filename),
            ),
        ],
        pdf_bytes,
    ))
}
