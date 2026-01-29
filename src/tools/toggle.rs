use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToggleBuiltinToolsParams {
    /// Action: "on" (enable), "off" (disable), or "status" (check current state)
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_action() -> String { "status".into() }

impl ToggleBuiltinToolsParams {
    pub fn validate(&self) -> Result<(), String> {
        let action = self.action.trim().to_lowercase();
        if !matches!(action.as_str(), "on" | "off" | "status") {
            return Err("Action must be 'on', 'off', or 'status'".into());
        }
        Ok(())
    }
}
