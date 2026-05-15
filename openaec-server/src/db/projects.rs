use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::state::{new_id, AppState};

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

pub async fn list_projects(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, name, description, created_at, updated_at FROM projects ORDER BY updated_at DESC",
    )?;

    let projects: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
                "updated_at": row.get::<_, String>(4)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(json!(projects)))
}

pub async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProject>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let id = new_id();
    let db = state.db.lock().unwrap();
    db.execute(
        "INSERT INTO projects (id, name, description) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, payload.name, payload.description],
    )?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "name": payload.name,
            "description": payload.description,
        })),
    ))
}

pub async fn get_project(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    let project = db.query_row(
        "SELECT id, name, description, created_at, updated_at FROM projects WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
                "updated_at": row.get::<_, String>(4)?,
            }))
        },
    )?;

    Ok(Json(project))
}

pub async fn update_project(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<CreateProject>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    let rows = db.execute(
        "UPDATE projects SET name = ?1, description = ?2, updated_at = datetime('now') WHERE id = ?3",
        rusqlite::params![payload.name, payload.description, id],
    )?;

    if rows == 0 {
        return Err(AppError::NotFound("Project not found".to_string()));
    }

    Ok(Json(json!({
        "id": id,
        "name": payload.name,
        "description": payload.description,
    })))
}

pub async fn delete_project(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let db = state.db.lock().unwrap();
    db.execute("DELETE FROM projects WHERE id = ?1", rusqlite::params![id])?;
    Ok(Json(json!({"detail": "Project verwijderd"})))
}
