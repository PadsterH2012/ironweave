use std::path::Path;
use std::process::Command;
use tracing::{info, warn, error};
use chrono::Utc;

use crate::db::DbPool;
use crate::models::project::Project;
use crate::models::mount::Mount;
use crate::error::{IronweaveError, Result};

pub struct SyncManager {
    db: DbPool,
    sync_base: String,
    jj_path: String,
}

fn find_jj() -> String {
    for path in &[
        "/home/paddy/.local/bin/jj",
        "/home/paddy/.cargo/bin/jj",
        "/usr/local/bin/jj",
    ] {
        if Path::new(path).exists() {
            return path.to_string();
        }
    }
    "jj".to_string() // fall back to PATH
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncSnapshot {
    pub change_id: String,
    pub description: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub sync_state: String,
    pub last_synced_at: Option<String>,
    pub sync_path: Option<String>,
    pub source: String,
}

impl SyncManager {
    pub fn new(db: DbPool, sync_base: String) -> Self {
        let jj_path = find_jj();
        info!(jj_path, "SyncManager initialised");
        Self { db, sync_base, jj_path }
    }

    fn sync_path(&self, project_id: &str) -> String {
        format!("{}/{}", self.sync_base, project_id)
    }

    fn source_path(&self, project: &Project) -> Result<String> {
        if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            let mount = Mount::get_by_id(&conn, mount_id)?;
            Ok(mount.local_mount_point.clone())
        } else {
            Ok(project.directory.clone())
        }
    }

    fn init_jj_repo(&self, sync_path: &str) -> Result<()> {
        std::fs::create_dir_all(sync_path)?;
        let output = Command::new(&self.jj_path)
            .args(["git", "init"])
            .current_dir(sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("already") {
                return Err(IronweaveError::Internal(format!("jj init failed: {}", stderr)));
            }
        }
        info!(sync_path, "jj repo initialised");
        Ok(())
    }

    fn run_rsync(&self, source: &str, dest: &str) -> Result<bool> {
        let source_with_slash = if source.ends_with('/') {
            source.to_string()
        } else {
            format!("{}/", source)
        };
        let output = Command::new("rsync")
            .args(["-az", "--delete", "--exclude", ".jj", "--exclude", ".git", "--itemize-changes", &source_with_slash, dest])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("rsync failed: {}", stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_changes = stdout.lines().any(|l| !l.trim().is_empty());
        Ok(has_changes)
    }

    fn jj_commit(&self, sync_path: &str, message: &str) -> Result<()> {
        let output = Command::new(&self.jj_path)
            .args(["describe", "-m", message])
            .current_dir(sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(sync_path, error = %stderr, "jj describe failed");
        }
        let output = Command::new(&self.jj_path)
            .arg("new")
            .current_dir(sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(sync_path, error = %stderr, "jj new failed");
        }
        Ok(())
    }

    pub fn sync_project(&self, project_id: &str) -> Result<SyncStatus> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let sync_path = self.sync_path(project_id);
        let source = self.source_path(&project)?;

        if project.mount_id.is_some() {
            // Check the mount is actually active, not just an empty directory
            let check = Command::new("mountpoint")
                .arg("-q")
                .arg(&source)
                .status();
            match check {
                Ok(s) if s.success() => {}
                _ => {
                    return Err(IronweaveError::Internal(
                        "Mount point not active. Ensure the mount is mounted before syncing.".to_string()
                    ));
                }
            }
        }

        let conn = self.db.lock().unwrap();
        Project::update_sync_state(&conn, project_id, "syncing", Some(&sync_path), None)?;
        drop(conn);

        if !Path::new(&sync_path).join(".jj").exists() {
            self.init_jj_repo(&sync_path)?;
        }

        match self.run_rsync(&source, &sync_path) {
            Ok(has_changes) => {
                let now = Utc::now().to_rfc3339();
                if has_changes {
                    let msg = format!("sync: {}", now);
                    self.jj_commit(&sync_path, &msg)?;
                    info!(project_id, "sync completed with changes");
                } else {
                    info!(project_id, "sync completed, no changes");
                }
                let conn = self.db.lock().unwrap();
                Project::update_sync_state(&conn, project_id, "idle", Some(&sync_path), Some(&now))?;
                Ok(SyncStatus {
                    sync_state: "idle".to_string(),
                    last_synced_at: Some(now),
                    sync_path: Some(sync_path),
                    source: "local".to_string(),
                })
            }
            Err(e) => {
                let conn = self.db.lock().unwrap();
                Project::update_sync_state(&conn, project_id, "error", Some(&sync_path), None)?;
                error!(project_id, error = %e, "sync failed");
                Err(e)
            }
        }
    }

    pub fn get_status(&self, project_id: &str) -> Result<SyncStatus> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        let source = if project.sync_path.is_some() && Path::new(project.sync_path.as_deref().unwrap_or("")).join(".jj").exists() {
            "local".to_string()
        } else if project.mount_id.is_some() {
            "sshfs".to_string()
        } else {
            "none".to_string()
        };
        Ok(SyncStatus {
            sync_state: project.sync_state,
            last_synced_at: project.last_synced_at,
            sync_path: project.sync_path,
            source,
        })
    }

    pub fn get_history(&self, project_id: &str, limit: usize) -> Result<Vec<SyncSnapshot>> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);
        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;
        if !Path::new(&sync_path).join(".jj").exists() {
            return Ok(vec![]);
        }
        let output = Command::new(&self.jj_path)
            .args([
                "log", "--no-graph",
                "-r", &format!("ancestors(@, {})", limit),
                "-T", r#"change_id.short(12) ++ "\t" ++ description.first_line() ++ "\t" ++ committer.timestamp() ++ "\n""#,
            ])
            .current_dir(&sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj log failed: {}", stderr)));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let snapshots: Vec<SyncSnapshot> = stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.len() >= 3 {
                    Some(SyncSnapshot {
                        change_id: parts[0].to_string(),
                        description: parts[1].to_string(),
                        timestamp: parts[2].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok(snapshots)
    }

    pub fn get_diff(&self, project_id: &str, change_id: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);
        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;
        if !change_id.chars().all(|c| c.is_alphanumeric()) {
            return Err(IronweaveError::Internal("Invalid change ID".to_string()));
        }
        let output = Command::new(&self.jj_path)
            .args(["diff", "-r", change_id])
            .current_dir(&sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj diff failed: {}", stderr)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn restore(&self, project_id: &str, change_id: &str) -> Result<()> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);
        let sync_path = project.sync_path.ok_or_else(|| {
            IronweaveError::Internal("No sync path configured".to_string())
        })?;
        if !change_id.chars().all(|c| c.is_alphanumeric()) {
            return Err(IronweaveError::Internal("Invalid change ID".to_string()));
        }
        let output = Command::new(&self.jj_path)
            .args(["restore", "--from", change_id])
            .current_dir(&sync_path)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(IronweaveError::Internal(format!("jj restore failed: {}", stderr)));
        }
        let msg = format!("restored from {}", change_id);
        self.jj_commit(&sync_path, &msg)?;
        Ok(())
    }

    pub fn browse_files(&self, project_id: &str, relative_path: &str) -> Result<Vec<crate::api::filesystem::BrowseEntry>> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let base = if let Some(ref sp) = project.sync_path {
            if Path::new(sp).exists() {
                sp.clone()
            } else if let Some(ref mount_id) = project.mount_id {
                let conn = self.db.lock().unwrap();
                Mount::get_by_id(&conn, mount_id)?.local_mount_point
            } else {
                project.directory.clone()
            }
        } else if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            Mount::get_by_id(&conn, mount_id)?.local_mount_point
        } else {
            project.directory.clone()
        };

        let full_path = if relative_path.is_empty() || relative_path == "/" {
            base.clone()
        } else {
            format!("{}/{}", base, relative_path.trim_start_matches('/'))
        };

        let canonical_base = Path::new(&base).canonicalize()
            .map_err(|_| IronweaveError::Internal("Base path not accessible".to_string()))?;
        let canonical_full = Path::new(&full_path).canonicalize()
            .map_err(|_| IronweaveError::NotFound("Path not found".to_string()))?;
        if !canonical_full.starts_with(&canonical_base) {
            return Err(IronweaveError::Internal("Path traversal not allowed".to_string()));
        }

        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(&canonical_full)
            .map_err(|_| IronweaveError::NotFound("Directory not found".to_string()))?;
        for entry in read_dir.flatten() {
            let file_type = entry.file_type().map_err(|e| IronweaveError::Internal(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            if file_type.is_dir() {
                entries.push(crate::api::filesystem::BrowseEntry { name, entry_type: "directory".to_string() });
            } else if file_type.is_file() {
                entries.push(crate::api::filesystem::BrowseEntry { name, entry_type: "file".to_string() });
            }
        }
        entries.sort_by(|a, b| {
            let type_cmp = a.entry_type.cmp(&b.entry_type);
            if type_cmp == std::cmp::Ordering::Equal {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                type_cmp
            }
        });
        Ok(entries)
    }

    pub fn read_file(&self, project_id: &str, relative_path: &str) -> Result<String> {
        let conn = self.db.lock().unwrap();
        let project = Project::get_by_id(&conn, project_id)?;
        drop(conn);

        let base = if let Some(ref sp) = project.sync_path {
            if Path::new(sp).exists() { sp.clone() } else { project.directory.clone() }
        } else if let Some(ref mount_id) = project.mount_id {
            let conn = self.db.lock().unwrap();
            Mount::get_by_id(&conn, mount_id)?.local_mount_point
        } else {
            project.directory.clone()
        };

        let full_path = format!("{}/{}", base, relative_path.trim_start_matches('/'));
        let canonical_base = Path::new(&base).canonicalize()
            .map_err(|_| IronweaveError::Internal("Base path not accessible".to_string()))?;
        let canonical_full = Path::new(&full_path).canonicalize()
            .map_err(|_| IronweaveError::NotFound("File not found".to_string()))?;
        if !canonical_full.starts_with(&canonical_base) {
            return Err(IronweaveError::Internal("Path traversal not allowed".to_string()));
        }
        let metadata = std::fs::metadata(&canonical_full)
            .map_err(|_| IronweaveError::NotFound("File not found".to_string()))?;
        if metadata.len() > 1_048_576 {
            return Err(IronweaveError::Internal("File too large (max 1MB)".to_string()));
        }
        std::fs::read_to_string(&canonical_full)
            .map_err(|e| IronweaveError::Internal(format!("Failed to read file: {}", e)))
    }
}
