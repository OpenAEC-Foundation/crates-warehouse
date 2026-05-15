use axum::{
    routing::{get, post, put},
    Router,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

use openaec_server::db;
use openaec_server::helpers::{template_loader, tenant_config};
use openaec_server::routes;
use openaec_server::state::{init_db, AppState};

fn cors_layer() -> CorsLayer {
    use axum::http::HeaderValue;

    let origins_str = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let default_origins = vec![
        "http://localhost:5173",
        "http://localhost:5174",
        "http://127.0.0.1:5173",
        "https://report.3bm.co.nl",
        "https://report.open-aec.com",
    ];

    let origins: Vec<HeaderValue> = if origins_str.is_empty() {
        default_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect()
    } else {
        origins_str
            .split(',')
            .map(|s| s.trim())
            .filter_map(|o| o.parse().ok())
            .collect()
    };

    use axum::http::header;

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
            header::HeaderName::from_static("x-api-key"),
        ])
        .allow_credentials(true)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Resolve static directory for SPA
    let static_dir = std::env::var("OPENAEC_STATIC_DIR").unwrap_or_else(|_| {
        for candidate in &["./static", "../frontend/dist", "/app/static"] {
            if std::path::Path::new(candidate).is_dir() {
                return candidate.to_string();
            }
        }
        "./static".to_string()
    });

    let spa_service = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(format!("{}/index.html", static_dir)));

    // Log tenant info
    let tc = tenant_config();
    if let Some(td) = tc.tenant_dir() {
        tracing::info!("Active tenant: {}", td.display());
    }
    if let Some(tr) = tc.tenants_root() {
        tracing::info!("Tenants root: {}", tr.display());
    }
    let loader = template_loader();
    let templates = loader.list_templates();
    tracing::info!("Loaded {} templates", templates.len());

    // Initialize database
    let db = Arc::new(Mutex::new(init_db()));
    let state = AppState { db };

    let app = Router::new()
        // Health
        .route("/api/health", get(routes::health::health))
        // Templates
        .route("/api/templates", get(routes::templates::list_templates))
        .route(
            "/api/templates/{name}/scaffold",
            get(routes::templates::get_scaffold),
        )
        // Brands & Stationery
        .route("/api/brands", get(routes::brands::list_brands))
        .route("/api/stationery", get(routes::stationery::list_stationery))
        // Generate — V1/V2 use legacy engine, /template uses V3 openaec-engine
        .route("/api/generate", post(routes::generate::generate))
        .route("/api/generate/v2", post(routes::generate::generate))
        .route(
            "/api/generate/template",
            post(routes::generate::generate_template),
        )
        // Validate
        .route("/api/validate", post(routes::validate::validate))
        // Projects CRUD
        .route(
            "/api/projects",
            get(db::projects::list_projects).post(db::projects::create_project),
        )
        .route(
            "/api/projects/{id}",
            get(db::projects::get_project)
                .put(db::projects::update_project)
                .delete(db::projects::delete_project),
        )
        // Reports CRUD
        .route(
            "/api/reports",
            get(db::reports::list_reports).post(db::reports::save_report),
        )
        .route(
            "/api/reports/{id}",
            get(db::reports::get_report)
                .put(db::reports::save_report_by_id)
                .delete(db::reports::delete_report),
        )
        .route("/api/reports/{id}/move", put(db::reports::move_report))
        // Admin
        .route("/api/admin/tenants", get(routes::admin::list_tenants))
        .route(
            "/api/admin/tenants/{tenant}/templates",
            get(routes::admin::list_tenant_templates),
        )
        .route(
            "/api/admin/tenants/{tenant}/brand",
            get(routes::admin::get_tenant_brand),
        )
        // Cloud storage
        .route(
            "/api/cloud/projects",
            get(routes::cloud::list_projects),
        )
        .route(
            "/api/cloud/projects/{project}/reports",
            get(routes::cloud::list_reports),
        )
        .route(
            "/api/cloud/projects/{project}/upload",
            post(routes::cloud::upload_report),
        )
        // Auth
        .route("/api/auth/me", get(routes::auth::auth_me))
        // Shared state
        .with_state(state)
        // CORS
        .layer(cors_layer())
        // SPA fallback
        .fallback_service(spa_service);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8001);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Rust API server listening on {}", addr);
    tracing::info!("Serving frontend from: {}", static_dir);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
