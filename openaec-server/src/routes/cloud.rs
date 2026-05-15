//! Cloud storage routes for Nextcloud project container model.
//!
//! Provides REST endpoints for listing projects, listing report files,
//! and uploading PDFs to a project's `reports/` directory. After upload,
//! the project manifest (`project.wefc`) is updated with a `WefcReport` entry.

use axum::extract::{Multipart, Path};
use axum::Json;
use serde::Serialize;
use tracing::{info, warn};

use crate::error::AppError;


// ── Response types ───────────────────────────────────────────

#[derive(Serialize)]
pub struct CloudProject {
    pub name: String,
}

#[derive(Serialize)]
pub struct CloudFile {
    pub name: String,
    pub size: u64,
    pub last_modified: String,
}

#[derive(Serialize)]
pub struct UploadResult {
    pub status: String,
    pub filename: String,
    pub project: String,
    pub path: String,
}

// ── Helpers ──────────────────────────────────────────────────

/// Get a configured CloudClient, or return an error if cloud is not configured.
fn get_cloud_client() -> Result<openaec_cloud::CloudClient, AppError> {
    let registry = openaec_cloud::TenantsRegistry::load_from_env()
        .map_err(|e| AppError::Internal(format!("failed to load tenant config: {e}")))?;

    // Use OPENAEC_TENANT env var to select the active tenant
    let tenant_slug =
        std::env::var("OPENAEC_TENANT").unwrap_or_else(|_| "3bm".to_string());

    let tenant = registry.get(&tenant_slug).ok_or_else(|| {
        AppError::Internal(format!(
            "tenant '{tenant_slug}' not found in tenants.json"
        ))
    })?;

    Ok(openaec_cloud::CloudClient::new(tenant, "openaec-reports"))
}

// ── Route handlers ───────────────────────────────────────────

/// `GET /api/cloud/projects` — List available project directories.
pub async fn list_projects() -> Result<Json<Vec<CloudProject>>, AppError> {
    let client = get_cloud_client()?;

    // Try volume mount first (sync, fast)
    let volume_projects = client.list_projects();
    if !volume_projects.is_empty() {
        let projects: Vec<CloudProject> = volume_projects
            .into_iter()
            .map(|p| CloudProject { name: p.name })
            .collect();
        return Ok(Json(projects));
    }

    // Fallback to WebDAV
    let webdav_projects = client
        .list_projects_webdav()
        .await
        .map_err(|e| AppError::Internal(format!("cloud error: {e}")))?;

    let projects: Vec<CloudProject> = webdav_projects
        .into_iter()
        .map(|p| CloudProject { name: p.name })
        .collect();

    Ok(Json(projects))
}

/// `GET /api/cloud/projects/{project}/reports` — List report files.
///
/// Tries new path (`reports/`) first, falls back to legacy
/// (`99_overige_documenten/reports/`).
pub async fn list_reports(
    Path(project): Path<String>,
) -> Result<Json<Vec<CloudFile>>, AppError> {
    let client = get_cloud_client()?;

    // list_files already handles new→legacy fallback internally
    let files = client.list_files(&project);
    let result: Vec<CloudFile> = files
        .into_iter()
        .filter(|f| f.name.ends_with(".pdf"))
        .map(|f| CloudFile {
            name: f.name,
            size: f.size,
            last_modified: f.last_modified,
        })
        .collect();

    Ok(Json(result))
}

/// `POST /api/cloud/projects/{project}/upload` — Upload a PDF to `reports/`.
///
/// Accepts multipart form data with a `file` field containing the PDF.
/// After upload, updates the project manifest (`project.wefc`) with a
/// `WefcReport` entry.
pub async fn upload_report(
    Path(project): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<UploadResult>, AppError> {
    let client = get_cloud_client()?;

    // Extract file from multipart
    let mut filename: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            filename = field
                .file_name()
                .map(|s: &str| s.to_string())
                .or_else(|| Some("report.pdf".to_string()));
            let bytes: axum::body::Bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("read error: {e}")))?;
            file_data = Some(bytes.to_vec());
        }
    }

    let filename = filename.ok_or_else(|| {
        AppError::BadRequest("missing 'file' field in multipart form".to_string())
    })?;
    let data = file_data.ok_or_else(|| {
        AppError::BadRequest("empty file data".to_string())
    })?;

    // Validate: must be a PDF
    if !filename.to_lowercase().ends_with(".pdf") {
        return Err(AppError::BadRequest(
            "only PDF files are allowed".to_string(),
        ));
    }

    let data_len = data.len();

    // Upload via WebDAV — always to new path (reports/)
    client
        .upload_file(&project, &filename, data)
        .await
        .map_err(|e| AppError::Internal(format!("upload failed: {e}")))?;

    info!(
        project = %project,
        filename = %filename,
        size = data_len,
        "uploaded report to cloud"
    );

    // Update project manifest with WefcReport entry
    let report_title = filename
        .strip_suffix(".pdf")
        .unwrap_or(&filename)
        .replace('_', " ");

    let now = now_iso8601();
    let report_object = serde_json::json!({
        "type": "WefcReport",
        "guid": uuid::Uuid::new_v4().to_string(),
        "name": report_title,
        "path": format!("reports/{filename}"),
        "status": "active",
        "created": now,
        "modified": now,
    });

    if let Err(e) = client
        .upsert_manifest_object(&project, report_object)
        .await
    {
        warn!(
            project = %project,
            error = %e,
            "failed to update manifest after upload — file was uploaded successfully"
        );
    }

    Ok(Json(UploadResult {
        status: "saved".to_string(),
        filename: filename.clone(),
        project: project.clone(),
        path: format!("reports/{filename}"),
    }))
}

/// Simple ISO 8601 UTC timestamp.
fn now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if remaining < diy {
            break;
        }
        remaining -= diy;
        y += 1;
    }

    let md: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0usize;
    for (i, &d) in md.iter().enumerate() {
        if remaining < d {
            m = i;
            break;
        }
        remaining -= d;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining + 1,
        hours,
        minutes,
        seconds
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
