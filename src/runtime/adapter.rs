use async_trait::async_trait;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCapabilities {
    pub streaming: bool,
    pub tool_use: bool,
    pub model_selection: bool,
    pub allowed_tools_filter: bool,
    pub dangerously_skip_permissions: bool,
    pub non_interactive: bool,
    pub supported_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightEnv {
    pub browsers_path: String,
    pub skip_download: bool,
}

impl Default for PlaywrightEnv {
    fn default() -> Self {
        Self {
            browsers_path: "/home/paddy/ironweave/browsers".to_string(),
            skip_download: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub working_directory: PathBuf,
    pub prompt: String,
    pub allowed_tools: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub extra_args: Option<Vec<String>>,
    /// Optional Playwright environment configuration
    pub playwright_env: Option<PlaywrightEnv>,
    /// Optional model override (e.g. "haiku", "sonnet", "opus")
    pub model: Option<String>,
}

impl AgentConfig {
    /// Returns the merged environment variables, including Playwright vars if configured.
    pub fn merged_env(&self) -> HashMap<String, String> {
        let mut env = self.environment.clone().unwrap_or_default();
        if let Some(ref pw) = self.playwright_env {
            env.insert(
                "PLAYWRIGHT_BROWSERS_PATH".to_string(),
                pw.browsers_path.clone(),
            );
            if pw.skip_download {
                env.insert(
                    "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD".to_string(),
                    "1".to_string(),
                );
            }
        }
        env
    }
}

pub struct SpawnedPty {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
}

#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn binary(&self) -> &str;
    fn capabilities(&self) -> RuntimeCapabilities;
    async fn check_available(&self) -> bool;
    fn build_command(&self, config: &AgentConfig) -> CommandBuilder;
    fn spawn_pty(&self, config: &AgentConfig, size: PtySize) -> crate::error::Result<SpawnedPty>;
}
