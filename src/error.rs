use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GrokError {
    #[error("Configuration missing: {0}")]
    ConfigMissing(String),

    #[error("Configuration invalid: {0}")]
    ConfigInvalid(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Max retries exceeded ({attempts} attempts): {last_error}")]
    MaxRetries { attempts: u32, last_error: String },

    #[error("Invalid parameter: {0}")]
    InvalidParam(String),

    #[error("Config file error at {path}: {message}")]
    ConfigFile { path: PathBuf, message: String },
}

pub type Result<T> = std::result::Result<T, GrokError>;
