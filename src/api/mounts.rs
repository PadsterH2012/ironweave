use axum::{extract::{Path, State}, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::mount::{Mount, CreateMount};

#[derive(Debug, Serialize)]
pub struct MountResponse {
    pub id: String,
    pub name: String,
    pub mount_type: String,
    pub remote_path: String,
    pub local_mount_point: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key: Option<String>,
    pub mount_options: Option<String>,
    pub auto_mount: bool,
    pub state: String,
    pub last_error: Option<String>,
    pub created_at: String,
    pub proxy_config_id: Option<String>,
    pub git_remote: Option<String>,
}

impl From<Mount> for MountResponse {
    fn from(m: Mount) -> Self {
        let redacted = m.redacted();
        Self {
            id: redacted.id,
            name: redacted.name,
            mount_type: redacted.mount_type,
            remote_path: redacted.remote_path,
            local_mount_point: redacted.local_mount_point,
            username: redacted.username,
            password: redacted.password,
            ssh_key: redacted.ssh_key,
            mount_options: redacted.mount_options,
            auto_mount: redacted.auto_mount,
            state: redacted.state,
            last_error: redacted.last_error,
            created_at: redacted.created_at,
            proxy_config_id: redacted.proxy_config_id,
            git_remote: redacted.git_remote,
        }
    }
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<MountResponse>> {
    let conn = state.db.lock().unwrap();
    let mounts = Mount::list(&conn).unwrap_or_default();
    Json(mounts.into_iter().map(MountResponse::from).collect())
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateMount>,
) -> Result<(StatusCode, Json<MountResponse>), StatusCode> {
    let conn = state.db.lock().unwrap();
    Mount::create(&conn, &input)
        .map(|m| (StatusCode::CREATED, Json(MountResponse::from(m))))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.db.lock().unwrap();
    Mount::delete(&conn, &id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn mount_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    {
        let conn = state.db.lock().unwrap();
        Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    }
    if let Some(ref mm) = state.mount_manager {
        mm.mount(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn unmount_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MountResponse>, StatusCode> {
    {
        let conn = state.db.lock().unwrap();
        Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    }
    if let Some(ref mm) = state.mount_manager {
        mm.unmount(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let conn = state.db.lock().unwrap();
    Mount::get_by_id(&conn, &id)
        .map(|m| Json(MountResponse::from(m)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(ref mm) = state.mount_manager {
        let s = mm.check_status(&id).map_err(|_| StatusCode::NOT_FOUND)?;
        Ok(Json(serde_json::json!({ "status": s })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateMount>,
) -> Result<Json<MountResponse>, StatusCode> {
    let conn = state.db.lock().unwrap();

    // Check if remote_path changed — if so, wipe jj history for linked projects
    let old = Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    let path_changed = old.remote_path != input.remote_path;

    let updated = Mount::update(&conn, &id, &input).map_err(|_| StatusCode::NOT_FOUND)?;

    if path_changed {
        if let Ok(projects) = crate::models::project::Project::list_by_mount(&conn, &id) {
            for project in &projects {
                if let Some(ref sync_path) = project.sync_path {
                    let jj_dir = std::path::Path::new(sync_path).join(".jj");
                    if jj_dir.exists() {
                        tracing::info!(
                            mount_id = %id,
                            project_id = %project.id,
                            sync_path,
                            "Remote path changed — wiping jj history"
                        );
                        let _ = std::fs::remove_dir_all(sync_path);
                    }
                }
                let _ = crate::models::project::Project::clear_sync_state(&conn, &project.id);
            }
        }
    }

    Ok(Json(MountResponse::from(updated)))
}

pub async fn duplicate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<MountResponse>), StatusCode> {
    let conn = state.db.lock().unwrap();
    let existing = Mount::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?;
    let input = CreateMount {
        name: format!("{} (copy)", existing.name),
        mount_type: existing.mount_type,
        remote_path: existing.remote_path,
        local_mount_point: existing.local_mount_point,
        username: existing.username,
        password: existing.password,
        ssh_key: existing.ssh_key,
        mount_options: existing.mount_options,
        auto_mount: Some(existing.auto_mount),
        proxy_config_id: existing.proxy_config_id,
        git_remote: existing.git_remote,
    };
    Mount::create(&conn, &input)
        .map(|m| (StatusCode::CREATED, Json(MountResponse::from(m))))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── SSH remote operations ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SshTestRequest {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: Option<String>,
    pub ssh_key: Option<String>,
    pub proxy_config_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteBrowseRequest {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: Option<String>,
    pub ssh_key: Option<String>,
    pub proxy_config_id: Option<String>,
    pub path: Option<String>,
}

/// Build SSH args for reaching a remote host, optionally through a proxy chain.
fn build_ssh_args(
    req_host: &str,
    req_port: u16,
    req_username: &str,
    req_password: Option<&str>,
    req_ssh_key: Option<&str>,
    proxy_hops: Option<&[crate::models::proxy_config::ProxyHop]>,
) -> (String, Vec<String>) {
    let mut ssh_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(), "ConnectTimeout=10".to_string(),
    ];

    // Add proxy chain if provided
    if let Some(hops) = proxy_hops {
        if !hops.is_empty() {
            let proxy_cmd = crate::mount::manager::MountManager::build_proxy_command(hops);
            ssh_args.push("-o".to_string());
            ssh_args.push(format!("ProxyCommand={}", proxy_cmd));
        }
    }

    if let Some(key) = req_ssh_key {
        ssh_args.push("-i".to_string());
        ssh_args.push(key.to_string());
    }

    if req_password.is_none() {
        ssh_args.push("-o".to_string());
        ssh_args.push("BatchMode=yes".to_string());
    }

    ssh_args.push("-p".to_string());
    ssh_args.push(req_port.to_string());
    ssh_args.push(format!("{}@{}", req_username, req_host));

    if let Some(pwd) = req_password {
        let mut sshpass_args = vec!["-p".to_string(), pwd.to_string(), "ssh".to_string()];
        sshpass_args.extend(ssh_args);
        ("sshpass".to_string(), sshpass_args)
    } else {
        ("ssh".to_string(), ssh_args)
    }
}

fn get_proxy_hops(state: &AppState, proxy_config_id: Option<&str>) -> Option<Vec<crate::models::proxy_config::ProxyHop>> {
    proxy_config_id.and_then(|pid| {
        let conn = state.db.lock().unwrap();
        crate::models::proxy_config::ProxyConfig::get_by_id(&conn, pid)
            .ok()
            .map(|pc| pc.hops)
    })
}

pub async fn test_ssh(
    State(state): State<AppState>,
    Json(req): Json<SshTestRequest>,
) -> Json<serde_json::Value> {
    let hops = get_proxy_hops(&state, req.proxy_config_id.as_deref());
    let (cmd, mut args) = build_ssh_args(
        &req.host,
        req.port.unwrap_or(22),
        &req.username,
        req.password.as_deref(),
        req.ssh_key.as_deref(),
        hops.as_deref(),
    );
    args.extend(["echo".to_string(), "ok".to_string()]);

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new(&cmd).args(&args).output(),
    ).await;

    match result {
        Ok(Ok(out)) if out.status.success() => {
            Json(serde_json::json!({ "success": true, "message": "SSH connection successful" }))
        }
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Json(serde_json::json!({ "success": false, "error": stderr.trim() }))
        }
        Ok(Err(e)) => {
            Json(serde_json::json!({ "success": false, "error": e.to_string() }))
        }
        Err(_) => {
            Json(serde_json::json!({ "success": false, "error": "Connection timed out (15s)" }))
        }
    }
}

pub async fn browse_remote(
    State(state): State<AppState>,
    Json(req): Json<RemoteBrowseRequest>,
) -> Json<serde_json::Value> {
    let path = req.path.as_deref().unwrap_or("/");
    let hops = get_proxy_hops(&state, req.proxy_config_id.as_deref());
    let (cmd, mut args) = build_ssh_args(
        &req.host,
        req.port.unwrap_or(22),
        &req.username,
        req.password.as_deref(),
        req.ssh_key.as_deref(),
        hops.as_deref(),
    );

    // List directory: type indicator (d/f), then name. Also detect .git
    let remote_cmd = format!(
        "if [ ! -d '{}' ]; then echo 'ERROR:not_a_directory'; exit 1; fi; \
         ls -1Ap '{}' 2>/dev/null | head -200; \
         echo '---GIT---'; \
         if [ -d '{}/.git' ] || git -C '{}' rev-parse --git-dir >/dev/null 2>&1; then \
           git -C '{}' remote get-url origin 2>/dev/null || echo 'NO_REMOTE'; \
         else echo 'NOT_GIT'; fi",
        path, path, path, path, path
    );
    args.push(remote_cmd);

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new(&cmd).args(&args).output(),
    ).await;

    match result {
        Ok(Ok(out)) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = stdout.splitn(2, "---GIT---").collect();
            let ls_output = parts.first().unwrap_or(&"");
            let git_output = parts.get(1).unwrap_or(&"").trim();

            let mut entries: Vec<serde_json::Value> = Vec::new();
            for line in ls_output.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                if line.ends_with('/') {
                    let name = line.trim_end_matches('/');
                    if name.starts_with('.') { continue; }
                    entries.push(serde_json::json!({ "name": name, "type": "directory" }));
                } else {
                    if line.starts_with('.') { continue; }
                    entries.push(serde_json::json!({ "name": line, "type": "file" }));
                }
            }

            let git_remote = if git_output == "NOT_GIT" || git_output.is_empty() {
                None
            } else if git_output == "NO_REMOTE" {
                Some("(git repo, no remote)".to_string())
            } else {
                Some(git_output.to_string())
            };

            Json(serde_json::json!({
                "path": path,
                "entries": entries,
                "git_remote": git_remote,
            }))
        }
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let err = if stdout.contains("ERROR:not_a_directory") {
                "Not a directory".to_string()
            } else {
                stderr.trim().to_string()
            };
            Json(serde_json::json!({ "error": err }))
        }
        Ok(Err(e)) => {
            Json(serde_json::json!({ "error": e.to_string() }))
        }
        Err(_) => {
            Json(serde_json::json!({ "error": "Connection timed out (15s)" }))
        }
    }
}
