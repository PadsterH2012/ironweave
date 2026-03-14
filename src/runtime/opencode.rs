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
        "/home/paddy/.opencode/bin/opencode"
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
                // Free built-in models
                "opencode/gpt-5-nano".into(),
                "opencode/mimo-v2-flash-free".into(),
                // Ollama (local — 12GB GPU, keep models ≤8B)
                "ollama/qwen3:8b".into(),
                "ollama/qwen2.5:7b".into(),
                "ollama/gemma3:4b".into(),
                "ollama/llama3.1:8b".into(),
                // OpenRouter
                "openrouter/anthropic/claude-sonnet-4".into(),
                "openrouter/google/gemini-2.5-pro".into(),
                "openrouter/meta-llama/llama-4-scout".into(),
                "openrouter/deepseek/deepseek-r1".into(),
                // Anthropic direct
                "anthropic/claude-sonnet-4-6".into(),
                "anthropic/claude-haiku-4-5-20251001".into(),
                // Google direct
                "google/gemini-2.5-pro".into(),
                "google/gemini-2.5-flash".into(),
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
        // Use script(1) to create a real PTY — bun binaries crash under portable_pty
        let mut cmd = CommandBuilder::new("/usr/bin/script");
        cmd.arg("-qc");
        let dir = config.working_directory.to_string_lossy();
        let prompt = config.prompt.replace('\'', "'\\''");
        let mut inner = format!("{} run", self.binary());
        if let Some(ref model) = config.model {
            inner.push_str(&format!(" --model '{}'", model));
        }
        inner.push_str(&format!(" --dir '{}' '{}'", dir, prompt));
        cmd.arg(&inner);
        cmd.arg("/dev/null");
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
