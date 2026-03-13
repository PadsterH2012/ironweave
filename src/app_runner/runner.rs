use std::collections::HashMap;
use std::process::{Command, Child, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::DbPool;

const PORT_RANGE_START: i32 = 8100;
const PORT_RANGE_END: i32 = 8199;

pub struct AppRunner {
    db: DbPool,
    children: Arc<Mutex<HashMap<String, Child>>>,
}

impl AppRunner {
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Reset any project_apps stuck in 'running' or 'starting' state on startup.
    /// After a service restart, those processes are gone — mark them stopped.
    pub fn cleanup_on_startup(&self) {
        let conn = self.db.lock().unwrap();
        let updated = conn.execute(
            "UPDATE project_apps SET state = 'stopped', pid = NULL, port = NULL, last_error = 'Service restarted' WHERE state IN ('running', 'starting')",
            [],
        ).unwrap_or(0);
        if updated > 0 {
            tracing::info!("App preview: reset {} stale app(s) to stopped on startup", updated);
        }
    }

    pub fn find_free_port(&self) -> Option<i32> {
        let conn = self.db.lock().unwrap();
        let used_ports: Vec<i32> = conn
            .prepare("SELECT port FROM project_apps WHERE state = 'running' AND port IS NOT NULL")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get(0))?
                    .collect::<std::result::Result<Vec<i32>, _>>()
            })
            .unwrap_or_default();

        (PORT_RANGE_START..=PORT_RANGE_END).find(|p| !used_ports.contains(p))
    }

    pub async fn start_app(
        &self,
        app_id: &str,
        project_dir: &str,
        detected: &super::detect::DetectedApp,
    ) -> Result<(i32, u32), String> {
        let port = self.find_free_port().ok_or("No free ports in range 8100-8199")?;

        let mut cmd = Command::new(&detected.command);
        cmd.current_dir(project_dir);

        for arg in &detected.args {
            cmd.arg(arg);
        }

        // Pass port
        if detected.port_via_env {
            cmd.env("PORT", port.to_string());
            if detected.args.contains(&"flask".to_string()) {
                cmd.env("FLASK_RUN_HOST", "0.0.0.0");
                cmd.env("FLASK_RUN_PORT", port.to_string());
                // Auto-detect FLASK_APP from project files
                let dir = std::path::Path::new(project_dir);
                if dir.join("app.py").exists() {
                    cmd.env("FLASK_APP", "app.py");
                } else if dir.join("main.py").exists() {
                    cmd.env("FLASK_APP", "main.py");
                }
            }
        } else {
            if detected.args.contains(&"runserver".to_string()) {
                cmd.arg(format!("0.0.0.0:{}", port));
            } else {
                cmd.arg(port.to_string());
                cmd.arg("--bind");
                cmd.arg("0.0.0.0");
            }
        }

        // Ensure user-local pip installs are findable
        if let Ok(path) = std::env::var("PATH") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/paddy".into());
            let local_bin = format!("{}/.local/bin", home);
            if !path.contains(&local_bin) {
                cmd.env("PATH", format!("{}:{}", local_bin, path));
            }
        }

        // Also set PYTHONPATH so python3 -m flask finds user-installed packages
        if let Ok(home) = std::env::var("HOME") {
            let site_packages = format!("{}/.local/lib/python3.13/site-packages", home);
            if std::path::Path::new(&site_packages).exists() {
                cmd.env("PYTHONPATH", site_packages);
            }
        }

        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;
        let pid = child.id();

        self.children.lock().await.insert(app_id.to_string(), child);

        Ok((port, pid))
    }

    pub async fn stop_app(&self, app_id: &str) -> Result<(), String> {
        let mut children = self.children.lock().await;
        if let Some(mut child) = children.remove(app_id) {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub async fn check_running(&self, app_id: &str) -> bool {
        let mut children = self.children.lock().await;
        if let Some(child) = children.get_mut(app_id) {
            match child.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }
}
