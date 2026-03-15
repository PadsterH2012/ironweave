use std::sync::{Arc, MutexGuard};
use axum::http::StatusCode;
use rusqlite::Connection;
use crate::auth::AuthConfig;
use crate::db::DbPool;
use crate::process::manager::ProcessManager;
use crate::orchestrator::runner::OrchestratorHandle;
use crate::runtime::RuntimeRegistry;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub process_manager: Arc<ProcessManager>,
    pub runtime_registry: Arc<RuntimeRegistry>,
    pub auth_config: Option<AuthConfig>,
    pub mount_manager: Option<Arc<crate::mount::manager::MountManager>>,
    pub filesystem_config: Option<crate::config::FilesystemConfig>,
    pub sync_manager: Option<Arc<crate::sync::manager::SyncManager>>,
    pub orchestrator: OrchestratorHandle,
    pub data_dir: std::path::PathBuf,
    pub app_runner: Arc<crate::app_runner::runner::AppRunner>,
}

impl AppState {
    /// Acquire database connection, returning 500 if the mutex is poisoned.
    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>, StatusCode> {
        self.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn browse_roots(&self) -> Vec<String> {
        self.filesystem_config
            .as_ref()
            .map(|c| c.browse_roots.clone())
            .unwrap_or_else(|| vec!["/home/paddy".to_string()])
    }
}
