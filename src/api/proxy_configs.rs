use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::Serialize;
use crate::state::AppState;
use crate::models::proxy_config::{ProxyConfig, CreateProxyConfig, UpdateProxyConfig};

#[derive(Debug, Serialize)]
pub struct ProxyConfigResponse {
    pub id: String,
    pub name: String,
    pub hops: Vec<crate::models::proxy_config::ProxyHop>,
    pub is_active: bool,
    pub created_at: String,
}

impl From<ProxyConfig> for ProxyConfigResponse {
    fn from(pc: ProxyConfig) -> Self {
        let redacted = pc.redacted();
        Self {
            id: redacted.id,
            name: redacted.name,
            hops: redacted.hops,
            is_active: redacted.is_active,
            created_at: redacted.created_at,
        }
    }
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<ProxyConfigResponse>>, StatusCode> {
    let conn = state.conn()?;
    let configs = ProxyConfig::list(&conn).unwrap_or_default();
    Ok(Json(configs.into_iter().map(ProxyConfigResponse::from).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateProxyConfig>,
) -> Result<(StatusCode, Json<ProxyConfigResponse>), StatusCode> {
    if input.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    for hop in &input.hops {
        if hop.host.trim().is_empty() || hop.username.trim().is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        if hop.port == 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let conn = state.conn()?;
    ProxyConfig::create(&conn, &input)
        .map(|pc| (StatusCode::CREATED, Json(ProxyConfigResponse::from(pc))))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProxyConfigResponse>, StatusCode> {
    let conn = state.conn()?;
    ProxyConfig::get_by_id(&conn, &id)
        .map(|pc| Json(ProxyConfigResponse::from(pc)))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProxyConfig>,
) -> Result<Json<ProxyConfigResponse>, StatusCode> {
    let conn = state.conn()?;
    ProxyConfig::update(&conn, &id, &input)
        .map(|pc| Json(ProxyConfigResponse::from(pc)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let conn = state.conn().map_err(|s| (s, "database unavailable".into()))?;
    ProxyConfig::delete(&conn, &id).map(|_| StatusCode::NO_CONTENT).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("referenced") {
            (StatusCode::CONFLICT, msg)
        } else if msg.contains("not found") {
            (StatusCode::NOT_FOUND, msg)
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        }
    })
}

pub async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pc = {
        let conn = state.conn()?;
        ProxyConfig::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?
    };

    if pc.hops.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "No hops configured"
        })));
    }

    // Test each hop sequentially, building up the ProxyJump chain
    for (i, hop) in pc.hops.iter().enumerate() {
        let mut ssh_args = vec![
            "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(), "ConnectTimeout=5".to_string(),
        ];

        let uses_password = hop.auth_type == "password" && hop.credential.is_some();
        if !uses_password {
            ssh_args.push("-o".to_string());
            ssh_args.push("BatchMode=yes".to_string());
        }

        // Build ProxyJump from all previous hops
        if i > 0 {
            let jump_chain: Vec<String> = pc.hops[..i]
                .iter()
                .map(|h| format!("{}@{}:{}", h.username, h.host, h.port))
                .collect();
            ssh_args.push("-o".to_string());
            ssh_args.push(format!("ProxyJump={}", jump_chain.join(",")));
        }

        ssh_args.push("-p".to_string());
        ssh_args.push(hop.port.to_string());
        ssh_args.push(format!("{}@{}", hop.username, hop.host));
        ssh_args.push("echo".to_string());
        ssh_args.push("ok".to_string());

        // Use sshpass for password-auth hops
        let (cmd, args) = if uses_password {
            let password = hop.credential.as_deref().unwrap_or("");
            let mut sshpass_args = vec![
                "-p".to_string(), password.to_string(),
                "ssh".to_string(),
            ];
            sshpass_args.extend(ssh_args);
            ("sshpass".to_string(), sshpass_args)
        } else {
            ("ssh".to_string(), ssh_args)
        };

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new(&cmd)
                .args(&args)
                .output()
        ).await;

        match output {
            Ok(Ok(out)) if out.status.success() => {
                // This hop passed, continue to next
            }
            Ok(Ok(out)) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "failed_hop": i,
                    "hops_tested": i + 1,
                    "error": format!("Hop {} ({}:{}) failed: {}", i + 1, hop.host, hop.port, stderr.trim())
                })));
            }
            Ok(Err(e)) => {
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "failed_hop": i,
                    "error": format!("SSH command failed at hop {}: {}", i + 1, e)
                })));
            }
            Err(_) => {
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "failed_hop": i,
                    "error": format!("Hop {} timed out (10s)", i + 1)
                })));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "hops_tested": pc.hops.len(),
        "message": format!("All {} hops reachable", pc.hops.len())
    })))
}
