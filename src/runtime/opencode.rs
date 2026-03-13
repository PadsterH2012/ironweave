use async_trait::async_trait;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::process::Command;

use super::adapter::*;

pub struct OpenCodeAdapter;

#[async_trait]
impl RuntimeAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "OpenCode"
    }

    fn binary(&self) -> &str {
        "opencode"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming: true,
            tool_use: true,
            model_selection: true,
            allowed_tools_filter: false,
            dangerously_skip_permissions: false,
            non_interactive: true,
            supported_models: vec![
                "claude-sonnet-4-6".into(),
                "gpt-4o".into(),
            ],
        }
    }

    async fn check_available(&self) -> bool {
        Command::new(self.binary())
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn build_command(&self, config: &AgentConfig) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(self.binary());
        cmd.arg("--non-interactive");
        if let Some(ref model) = config.model {
            cmd.arg("--model");
            cmd.arg(model);
        }
        cmd.arg(&config.prompt);
        cmd.cwd(&config.working_directory);
        for (k, v) in config.merged_env() {
            cmd.env(k, v);
        }
        cmd
    }

    fn spawn_pty(&self, config: &AgentConfig, size: PtySize) -> crate::error::Result<SpawnedPty> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(size)
            .map_err(|e| crate::error::IronweaveError::Internal(format!("PTY open failed: {}", e)))?;
        let cmd = self.build_command(config);
        let child = pair.slave
            .spawn_command(cmd)
            .map_err(|e| crate::error::IronweaveError::Internal(format!("PTY spawn failed: {}", e)))?;
        drop(pair.slave);
        Ok(SpawnedPty { master: pair.master, child })
    }
}
