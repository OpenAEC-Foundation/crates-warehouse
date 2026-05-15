use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use crate::helpers::tenant_config;

/// Reads X-Authentik-* headers from Caddy forward auth proxy.
pub async fn auth_me(headers: HeaderMap) -> Json<Value> {
    let get_header = |name: &str| -> String {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    };

    let username = get_header("X-Authentik-Username");
    let email = get_header("X-Authentik-Email");
    let name = get_header("X-Authentik-Name");
    let uid = get_header("X-Authentik-Uid");
    let groups = get_header("X-Authentik-Groups");

    let tc = tenant_config();
    let tenant_name = tc
        .tenant_dir()
        .and_then(|p| p.file_name().map(|n| n.to_os_string()))
        .and_then(|n| n.into_string().ok())
        .unwrap_or_else(|| "3bm_cooperatie".to_string());

    let is_authenticated = !username.is_empty();

    Json(json!({
        "user": {
            "id": uid,
            "username": username,
            "email": email,
            "display_name": if name.is_empty() { username.clone() } else { name },
            "role": if groups.contains("admins") { "admin" } else { "user" },
            "tenant": tenant_name,
            "is_active": is_authenticated,
            "groups": groups,
            "auth_provider": "authentik"
        }
    }))
}
