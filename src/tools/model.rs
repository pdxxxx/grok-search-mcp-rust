use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SwitchModelParams {
    /// Model name to switch to (e.g., "grok-4-fast", "grok-2-latest")
    pub model: String,
}

impl SwitchModelParams {
    pub fn validate(&self) -> Result<(), String> {
        let model = self.model.trim();
        if model.is_empty() {
            return Err("Model name cannot be empty".into());
        }
        if model.len() > 100 {
            return Err("Model name exceeds 100 characters".into());
        }
        Ok(())
    }
}
