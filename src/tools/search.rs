use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchParams {
    /// Search query (max 2000 characters)
    pub query: String,

    /// Platform hint (e.g., "twitter", "github", "reddit")
    #[serde(default)]
    pub platform: String,

    /// Minimum number of results (1-50, default 3)
    #[serde(default = "default_min_results")]
    pub min_results: u32,

    /// Maximum number of results (1-100, default 10)
    #[serde(default = "default_max_results")]
    pub max_results: u32,
}

fn default_min_results() -> u32 { 3 }
fn default_max_results() -> u32 { 10 }

impl WebSearchParams {
    pub fn validate(&self) -> Result<(), String> {
        let query = self.query.trim();
        if query.is_empty() {
            return Err("Query cannot be empty".into());
        }
        if query.len() > 2000 {
            return Err("Query exceeds 2000 characters".into());
        }
        if self.min_results < 1 || self.min_results > 50 {
            return Err("min_results must be between 1 and 50".into());
        }
        if self.max_results < 1 || self.max_results > 100 {
            return Err("max_results must be between 1 and 100".into());
        }
        if self.min_results > self.max_results {
            return Err("min_results cannot be greater than max_results".into());
        }
        Ok(())
    }
}
