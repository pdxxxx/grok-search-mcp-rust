use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchParams {
    /// URL to fetch (must be http or https)
    pub url: String,
}

impl WebFetchParams {
    pub fn validate(&self) -> Result<(), String> {
        let url = self.url.trim();
        if url.is_empty() {
            return Err("URL cannot be empty".into());
        }
        if url.len() > 2048 {
            return Err("URL exceeds 2048 characters".into());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("URL must use http or https scheme".into());
        }
        Ok(())
    }
}
