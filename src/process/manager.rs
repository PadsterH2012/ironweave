use std::collections::HashMap;
use std::io::Write as _;
use std::sync::Arc;
use tokio::sync::RwLock;
use portable_pty::{Child, MasterPty, PtySize};

use crate::runtime::adapter::AgentConfig;
use crate::runtime::RuntimeRegistry;

pub struct ManagedAgent {
    pub session_id: String,
    pub runtime: String,
    pub config: AgentConfig,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Option<Box<dyn std::io::Write + Send>>,
}

pub struct ProcessManager {
    registry: Arc<RuntimeRegistry>,
    agents: Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<ManagedAgent>>>>>,
}

impl ProcessManager {
    pub fn new(registry: Arc<RuntimeRegistry>) -> Self {
        Self {
            registry,
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn spawn_agent(
        &self,
        session_id: &str,
        runtime_name: &str,
        config: AgentConfig,
        size: PtySize,
    ) -> crate::error::Result<()> {
        let adapter = self.registry.get(runtime_name).ok_or_else(|| {
            crate::error::IronweaveError::NotFound(format!("runtime adapter: {}", runtime_name))
        })?;

        // Clone config before moving into spawn_blocking (AgentConfig is Clone)
        let config_for_agent = config.clone();

        // portable-pty is synchronous, so spawn on a blocking thread
        let spawned = tokio::task::spawn_blocking(move || {
            adapter.spawn_pty(&config, size)
        })
        .await
        .map_err(|e| crate::error::IronweaveError::Internal(format!("spawn_blocking join error: {}", e)))??;

        let writer = spawned.master.take_writer().ok();

        // Start background log writer — tee PTY output to data/agent-logs/{session_id}.log
        if let Ok(reader) = spawned.master.try_clone_reader() {
            let log_dir = std::path::PathBuf::from("data/agent-logs");
            let _ = std::fs::create_dir_all(&log_dir);
            let log_path = log_dir.join(format!("{}.log", session_id));
            tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let mut reader = reader;
                let mut buf = [0u8; 4096];
                if let Ok(mut file) = std::fs::File::create(&log_path) {
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => { let _ = std::io::Write::write_all(&mut file, &buf[..n]); }
                            Err(_) => break,
                        }
                    }
                }
            });
        }

        let managed = ManagedAgent {
            session_id: session_id.to_string(),
            runtime: runtime_name.to_string(),
            config: config_for_agent,
            master: spawned.master,
            child: spawned.child,
            writer,
        };

        self.agents
            .write()
            .await
            .insert(session_id.to_string(), Arc::new(tokio::sync::Mutex::new(managed)));

        Ok(())
    }

    pub async fn get_agent(&self, session_id: &str) -> Option<Arc<tokio::sync::Mutex<ManagedAgent>>> {
        self.agents.read().await.get(session_id).cloned()
    }

    pub async fn stop_agent(&self, session_id: &str) -> crate::error::Result<()> {
        let agent = self.agents.write().await.remove(session_id).ok_or_else(|| {
            crate::error::IronweaveError::NotFound(format!("agent session: {}", session_id))
        })?;

        // Kill the child process
        let mut locked = agent.lock().await;
        locked.child.kill().map_err(|e| {
            crate::error::IronweaveError::Internal(format!("failed to kill agent: {}", e))
        })?;

        Ok(())
    }

    pub async fn list_active(&self) -> Vec<(String, String)> {
        let agents = self.agents.read().await;
        let mut result = Vec::with_capacity(agents.len());
        for (id, agent) in agents.iter() {
            let locked = agent.lock().await;
            result.push((id.clone(), locked.runtime.clone()));
        }
        result
    }

    pub async fn write_to_agent(&self, session_id: &str, data: &[u8]) -> crate::error::Result<()> {
        let agent = self.agents.read().await.get(session_id).cloned().ok_or_else(|| {
            crate::error::IronweaveError::NotFound(format!("agent session: {}", session_id))
        })?;
        let mut locked = agent.lock().await;
        let writer = locked.writer.as_mut().ok_or_else(|| {
            crate::error::IronweaveError::Internal("PTY writer not available (taken by WebSocket)".into())
        })?;
        writer.write_all(data).map_err(|e| {
            crate::error::IronweaveError::Internal(format!("failed to write to agent PTY: {}", e))
        })?;
        Ok(())
    }

    pub async fn remove_agent(&self, session_id: &str) {
        self.agents.write().await.remove(session_id);
    }

    /// Check if an agent process has exited.
    /// Returns `Some(true)` for exit code 0, `Some(false)` for non-zero, `None` if still running.
    pub async fn check_agent_exit(&self, session_id: &str) -> Option<bool> {
        let agent = self.agents.read().await.get(session_id).cloned()?;
        let mut locked = agent.lock().await;
        match locked.child.try_wait() {
            Ok(Some(status)) => Some(status.success()),
            Ok(None) => None,          // still running
            Err(_) => Some(false),      // treat errors as failure
        }
    }
}
