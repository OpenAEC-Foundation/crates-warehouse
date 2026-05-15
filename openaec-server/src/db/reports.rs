use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::AppError;
use crate::state::{new_id, AppState};

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveReport {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub content: Value,
}

pub async fn list_reports(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();

    let (sql, param): (String, Option<String>) = if let Some(project_id) = params.get("project_id")
    {
        (
            "SELECT id, title, template, project_id, created_at, updated_at FROM reports WHERE project_id = ?1 ORDER BY updated_at DESC".to_string(),
            Some(project_id.clone()),
        )
    } else {
        (
            "SELECT id, title, template, project_id, created_at, updated_at FROM reports ORDER BY updated_at DESC".to_string(),
            None,
        )
    };

    let mut stmt = db.prepare(&sql)?;

    let row_to_json = |row: &rusqlite::Row| -> rusqlite::Result<Value> {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "title": row.get::<_, String>(1)?,
            "template": row.get::<_, String>(2)?,
            "project_id": row.get::<_, Option<String>>(3)?,
            "created_at": row.get::<_, String>(4)?,
            "updated_at": row.get::<_, String>(5)?,
        }))
    };

    let reports: Vec<Value> = if let Some(ref pid) = param {
        stmt.query_map(rusqlite::params![pid], row_to_json)
    } else {
        stmt.query_map([], row_to_json)
    }?
    .filter_map(|r| r.ok())
    .collect();

    Ok(Json(json!(reports)))
}

pub async fn save_report(
    State(state): State<AppState>,
    Json(payload): Json<SaveReport>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let id = if payload.id.is_empty() {
        new_id()
    } else {
        payload.id.clone()
    };
    let content_str = serde_json::to_string(&payload.content).unwrap_or_else(|_| "{}".to_string());

    let db = state.db.lock().unwrap();
    db.execute(
        "INSERT INTO reports (id, title, template, project_id, content) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET title=?2, template=?3, project_id=?4, content=?5, updated_at=datetime('now')",
        rusqlite::params![id, payload.title, payload.template, payload.project_id, content_str],
    )?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "title": payload.title,
            "template": payload.template,
            "project_id": payload.project_id,
        })),
    ))
}

pub async fn get_report(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    let report = db.query_row(
        "SELECT id, title, template, project_id, content, created_at, updated_at FROM reports WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            let content_str: String = row.get(4)?;
            let content: Value = serde_json::from_str(&content_str).unwrap_or(json!({}));
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "template": row.get::<_, String>(2)?,
                "project_id": row.get::<_, Option<String>>(3)?,
                "content": content,
                "created_at": row.get::<_, String>(5)?,
                "updated_at": row.get::<_, String>(6)?,
            }))
        },
    )?;

    Ok(Json(report))
}

pub async fn save_report_by_id(
    state: State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(mut payload): Json<SaveReport>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    payload.id = id;
    save_report(state, Json(payload)).await
}

pub async fn delete_report(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    db.execute("DELETE FROM reports WHERE id = ?1", rusqlite::params![id])?;
    Ok(Json(json!({"detail": "Report verwijderd"})))
}

pub async fn move_report(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let project_id = payload
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let db = state.db.lock().unwrap();
    db.execute(
        "UPDATE reports SET project_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![project_id, id],
    )?;

    Ok(Json(json!({"detail": "Report verplaatst"})))
}
