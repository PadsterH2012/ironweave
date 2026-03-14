mod api;
mod auth;
mod config;
mod db;
mod error;
mod models;
mod orchestrator;
mod process;
mod runtime;
mod state;
mod worktree;
mod mount;
mod sync;
mod app_runner;

use std::sync::Arc;
use std::net::SocketAddr;
use axum::{Router, middleware, routing::{get, post, put, patch, delete}};
use axum::response::{Response, IntoResponse};
use axum::http::{StatusCode, header};
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

use crate::config::Config;
use crate::runtime::RuntimeRegistry;
use crate::process::manager::ProcessManager;
use crate::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config: Config = if std::path::Path::new("ironweave.toml").exists() {
        let content = std::fs::read_to_string("ironweave.toml").expect("Failed to read config");
        toml::from_str(&content).expect("Failed to parse config")
    } else {
        Config::default()
    };
    let db = db::init_db(&config.database_path).expect("Failed to init database");

    // Run session table migration
    auth::create_sessions_table(&db).expect("Failed to create sessions table");

    // Seed default settings from config (only on first run)
    {
        use crate::models::setting::Setting;
        let conn = db.lock().unwrap();
        if let Some(ref fs) = config.filesystem {
            let roots = serde_json::to_string(&fs.browse_roots).unwrap_or_else(|_| "[]".to_string());
            Setting::seed(&conn, "browse_roots", &roots, "general").unwrap_or(());
            Setting::seed(&conn, "mount_base", &fs.mount_base, "general").unwrap_or(());
            if let Some(mins) = fs.idle_unmount_minutes {
                Setting::seed(&conn, "idle_unmount_minutes", &mins.to_string(), "general").unwrap_or(());
            }
        }
        if let Some(ref sec) = config.security {
            Setting::seed(&conn, "master_key", &sec.master_key, "general").unwrap_or(());
        }
    }

    // Seed team templates
    {
        let conn = db.lock().unwrap();
        crate::db::seeds::seed_team_templates(&conn).unwrap_or_else(|e| {
            tracing::warn!("Failed to seed team templates: {}", e);
        });
    }

    let registry = Arc::new(RuntimeRegistry::new());
    let process_manager = Arc::new(ProcessManager::new(registry.clone()));

    let auth_config = config.auth.clone();

    let mount_manager = config.filesystem.as_ref().map(|fs_config| {
        Arc::new(mount::manager::MountManager::new(db.clone(), fs_config.clone()))
    });

    let sync_manager = config.filesystem.as_ref().map(|fs_config| {
        let sync_base = format!("{}/sync", fs_config.mount_base.trim_end_matches('/'));
        Arc::new(sync::manager::SyncManager::new(db.clone(), sync_base))
    });

    // Orchestrator
    let (orch_tx, orch_rx) = tokio::sync::mpsc::channel(64);
    let orchestrator_handle = crate::orchestrator::runner::OrchestratorHandle::new(orch_tx);

    let worktree_base = std::path::PathBuf::from("/home/paddy/ironweave-worktrees");
    let mut orch_runner = crate::orchestrator::runner::OrchestratorRunner::new(
        orch_rx,
        db.clone(),
        process_manager.clone(),
        registry.clone(),
        worktree_base,
    ).with_build_server(config.build_server.clone());
    tokio::spawn(async move {
        orch_runner.restore_running_instances().await;
        orch_runner.run().await;
    });

    let app_runner = Arc::new(app_runner::runner::AppRunner::new(db.clone()));
    app_runner.cleanup_on_startup();

    let state = AppState {
        db: db.clone(),
        process_manager,
        runtime_registry: registry,
        auth_config: auth_config.clone(),
        mount_manager: mount_manager.clone(),
        filesystem_config: config.filesystem.clone(),
        sync_manager,
        orchestrator: orchestrator_handle,
        data_dir: config.data_dir.clone(),
        app_runner,
    };

    let mut app = Router::new()
        // Health
        .route("/api/health", get(|| async { "ok" }))
        // Auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        // Projects
        .route("/api/projects", get(api::projects::list).post(api::projects::create))
        .route("/api/projects/{id}", get(api::projects::get).put(api::projects::update).delete(api::projects::delete))
        // Team templates (must be before /api/teams/{tid} routes)
        .route("/api/teams/templates", get(api::teams::list_templates))
        .route("/api/projects/{pid}/teams/templates", get(api::teams::list_project_templates))
        .route("/api/projects/{pid}/teams/from-template/{tid}", post(api::teams::clone_template))
        // Teams
        .route("/api/projects/{pid}/teams", get(api::teams::list).post(api::teams::create))
        .route("/api/projects/{pid}/teams/{id}", get(api::teams::get).delete(api::teams::delete))
        .route("/api/projects/{pid}/teams/{id}/activate", put(api::teams::activate))
        .route("/api/projects/{pid}/teams/{id}/deactivate", put(api::teams::deactivate))
        .route("/api/projects/{pid}/teams/{id}/config", put(api::teams::update_config))
        .route("/api/projects/{pid}/teams/{id}/status", get(api::teams::team_status))
        // Team Agent Slots
        .route("/api/teams/{tid}/slots", get(api::teams::list_slots).post(api::teams::create_slot))
        .route("/api/teams/{tid}/slots/{id}", put(api::teams::update_slot).delete(api::teams::delete_slot))
        // Issues
        .route("/api/projects/{pid}/issues", get(api::issues::list).post(api::issues::create))
        .route("/api/projects/{pid}/issues/ready", get(api::issues::ready))
        .route("/api/projects/{pid}/issues/{id}", get(api::issues::get).patch(api::issues::update).delete(api::issues::delete))
        .route("/api/projects/{pid}/issues/{id}/claim", post(api::issues::claim))
        .route("/api/projects/{pid}/issues/{id}/children", get(api::issues::children))
        .route("/api/projects/{pid}/issues/{id}/unclaim", post(api::issues::unclaim))
        // Attachments
        .route("/api/projects/{pid}/issues/{id}/attachments", get(api::attachments::list).post(api::attachments::upload))
        .route("/api/attachments/{id}/download", get(api::attachments::download))
        // Agents
        .route("/api/agents", get(api::agents::list))
        .route("/api/agents/spawn", post(api::agents::spawn))
        .route("/api/agents/{id}", get(api::agents::get_agent))
        .route("/api/agents/{id}/stop", post(api::agents::stop))
        // Agent WebSocket
        .route("/ws/agents/{id}", get(api::agents::ws_agent_output))
        // Workflows
        .route("/api/projects/{pid}/workflows", get(api::workflows::list_definitions).post(api::workflows::create_definition))
        .route("/api/projects/{pid}/workflows/{id}", get(api::workflows::get_definition))
        .route("/api/workflows/{wid}/instances", get(api::workflows::list_instances).post(api::workflows::create_instance))
        .route("/api/workflows/{wid}/instances/{iid}/stages/{sid}/approve", post(api::workflows::approve_gate))
        // Dashboard
        .route("/api/dashboard", get(api::dashboard::stats))
        .route("/api/dashboard/activity", get(api::dashboard::activity))
        .route("/api/dashboard/metrics", get(api::dashboard::metrics))
        .route("/api/dashboard/system", get(api::dashboard::system))
        // Filesystem browser
        .route("/api/filesystem/browse", get(api::filesystem::browse))
        // Mounts
        .route("/api/mounts", get(api::mounts::list).post(api::mounts::create))
        .route("/api/mounts/{id}", get(api::mounts::get).put(api::mounts::update).delete(api::mounts::delete))
        .route("/api/mounts/{id}/mount", post(api::mounts::mount_action))
        .route("/api/mounts/{id}/unmount", post(api::mounts::unmount_action))
        .route("/api/mounts/{id}/status", get(api::mounts::status))
        .route("/api/mounts/{id}/duplicate", post(api::mounts::duplicate))
        .route("/api/mounts/test-ssh", post(api::mounts::test_ssh))
        .route("/api/mounts/browse-remote", post(api::mounts::browse_remote))
        // Settings
        .route("/api/settings", get(api::settings::list))
        .route("/api/settings/{key}", get(api::settings::get).put(api::settings::upsert).delete(api::settings::delete))
        // Proxy Configs
        .route("/api/proxy-configs", get(api::proxy_configs::list).post(api::proxy_configs::create))
        .route("/api/proxy-configs/{id}", get(api::proxy_configs::get).put(api::proxy_configs::update).delete(api::proxy_configs::delete))
        .route("/api/proxy-configs/{id}/test", post(api::proxy_configs::test_connection))
        // Project sync
        .route("/api/projects/{id}/sync", post(api::sync::trigger_sync))
        .route("/api/projects/{id}/sync/status", get(api::sync::get_status))
        .route("/api/projects/{id}/sync/history", get(api::sync::get_history))
        .route("/api/projects/{id}/sync/diff/{change_id}", get(api::sync::get_diff))
        .route("/api/projects/{id}/sync/restore", post(api::sync::restore))
        .route("/api/projects/{id}/files", get(api::sync::browse_files))
        .route("/api/projects/{id}/files/content", get(api::sync::read_file))
        // Project App Preview
        .route("/api/projects/{id}/app/start", post(api::project_apps::start))
        .route("/api/projects/{id}/app/stop", post(api::project_apps::stop))
        .route("/api/projects/{id}/app/status", get(api::project_apps::status))
        // Plan import
        .route("/api/projects/{pid}/import-plan", post(api::plan_import::import_plan))
        // Merge queue
        .route("/api/projects/{pid}/merge-queue", get(api::merge_queue::list_queue))
        .route("/api/projects/{pid}/merge-queue/{id}/approve", post(api::merge_queue::approve_merge))
        // Runtimes
        .route("/api/runtimes", get(api::runtimes::list))
        // Loom
        .route("/api/projects/{pid}/loom", get(api::loom::list_by_project))
        .route("/api/teams/{tid}/loom", get(api::loom::list_by_team))
        .route("/api/loom", get(api::loom::list_recent).post(api::loom::create));

    // Only add auth middleware if auth is configured
    if auth_config.is_some() {
        tracing::info!("Authentication enabled");
        app = app.layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));
    } else {
        tracing::info!("Authentication disabled (no [auth] config)");
    }

    if let (Some(fs_config), Some(mm)) = (&config.filesystem, &mount_manager) {
        mount::idle_monitor::spawn_idle_monitor(db.clone(), fs_config.clone(), mm.clone());
        tracing::info!("Mount idle monitor started");
    }

    let app = app
        .fallback(serve_frontend)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    if let Some(tls) = &config.tls {
        tracing::info!("Ironweave listening on https://{}", addr);
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            &tls.cert_path,
            &tls.key_path,
        )
        .await
        .expect("Failed to load TLS certificates");

        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        tracing::info!("Ironweave listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

async fn serve_frontend(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(file) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.into_owned(),
        )
            .into_response()
    } else {
        // SPA fallback — serve index.html
        match FrontendAssets::get("index.html") {
            Some(file) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html")],
                file.data.into_owned(),
            )
                .into_response(),
            None => (StatusCode::NOT_FOUND, "Frontend not built").into_response(),
        }
    }
}
