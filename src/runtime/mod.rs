pub mod adapter;
pub mod claude;
pub mod gemini;
pub mod opencode;

use adapter::RuntimeAdapter;
use std::collections::HashMap;
use std::sync::Arc;

pub struct RuntimeRegistry {
    adapters: HashMap<String, Arc<dyn RuntimeAdapter>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Arc<dyn RuntimeAdapter>> = HashMap::new();
        adapters.insert("claude".to_string(), Arc::new(claude::ClaudeAdapter));
        adapters.insert(
            "opencode".to_string(),
            Arc::new(opencode::OpenCodeAdapter),
        );
        adapters.insert("gemini".to_string(), Arc::new(gemini::GeminiAdapter));
        Self { adapters }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn RuntimeAdapter>> {
        self.adapters.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}
