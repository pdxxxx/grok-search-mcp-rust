use crate::error::{GrokError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_MODEL: &str = "grok-4-fast";
const CONFIG_DIR_NAME: &str = "grok-search";
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub debug_enabled: bool,
    pub retry_max_attempts: u32,
    pub retry_multiplier: f64,
    pub retry_max_wait: u64,
    pub log_level: String,
    pub log_dir: Option<String>,
    pub builtin_tools_disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    builtin_tools_disabled: Option<bool>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let api_url = env_required("GROK_API_URL")?;
        validate_url(&api_url)?;

        let api_key = env_required("GROK_API_KEY")?.trim().to_string();
        if api_key.is_empty() {
            return Err(GrokError::ConfigInvalid("GROK_API_KEY cannot be empty".into()));
        }

        let persisted = read_persisted_config();

        let model = persisted.model.clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| env_opt("GROK_MODEL"))
            .unwrap_or_else(|| DEFAULT_MODEL.into());

        Ok(Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            debug_enabled: env_bool("GROK_DEBUG"),
            retry_max_attempts: env_u32_range("GROK_RETRY_MAX_ATTEMPTS", 3, 1, 10)?,
            retry_multiplier: env_f64_range("GROK_RETRY_MULTIPLIER", 1.0, 0.1, 10.0)?,
            retry_max_wait: env_u64_range("GROK_RETRY_MAX_WAIT", 10, 1, 300)?,
            log_level: env_opt("GROK_LOG_LEVEL").unwrap_or_else(|| "INFO".into()).to_uppercase(),
            log_dir: env_opt("GROK_LOG_DIR"),
            builtin_tools_disabled: persisted.builtin_tools_disabled.unwrap_or(false),
        })
    }

    pub fn save_model(model: &str) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            return Err(GrokError::ConfigInvalid("Model name cannot be empty".into()));
        }
        let mut cfg = read_persisted_config();
        cfg.model = Some(model.into());
        write_config_atomic(&cfg)
    }

    pub fn save_builtin_tools_disabled(disabled: bool) -> Result<()> {
        let mut cfg = read_persisted_config();
        cfg.builtin_tools_disabled = Some(disabled);
        write_config_atomic(&cfg)
    }

    pub fn mask_api_key(&self) -> String {
        mask_key(&self.api_key)
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_DIR_NAME)
    }

    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join(CONFIG_FILE_NAME)
    }
}

fn env_required(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| {
        GrokError::ConfigMissing(format!(
            "{name} not configured.\nPlease configure with:\nclaude mcp add-json grok-search --scope user \
            '{{\"type\":\"stdio\",\"command\":\"grok-search-mcp\",\"env\":{{\"GROK_API_URL\":\"your-url\",\"GROK_API_KEY\":\"your-key\"}}}}'"
        ))
    })
}

fn env_opt(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

fn env_u32_range(name: &str, default: u32, min: u32, max: u32) -> Result<u32> {
    let Some(raw) = env_opt(name) else { return Ok(default) };
    let val: u32 = raw.parse().map_err(|_| {
        GrokError::ConfigInvalid(format!("{name} must be an integer between {min} and {max}"))
    })?;
    if !(min..=max).contains(&val) {
        return Err(GrokError::ConfigInvalid(format!("{name} must be an integer between {min} and {max}")));
    }
    Ok(val)
}

fn env_u64_range(name: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    let Some(raw) = env_opt(name) else { return Ok(default) };
    let val: u64 = raw.parse().map_err(|_| {
        GrokError::ConfigInvalid(format!("{name} must be an integer between {min} and {max}"))
    })?;
    if !(min..=max).contains(&val) {
        return Err(GrokError::ConfigInvalid(format!("{name} must be an integer between {min} and {max}")));
    }
    Ok(val)
}

fn env_f64_range(name: &str, default: f64, min: f64, max: f64) -> Result<f64> {
    let Some(raw) = env_opt(name) else { return Ok(default) };
    let val: f64 = raw.parse().map_err(|_| {
        GrokError::ConfigInvalid(format!("{name} must be a number between {min} and {max}"))
    })?;
    if !val.is_finite() || val < min || val > max {
        return Err(GrokError::ConfigInvalid(format!("{name} must be a number between {min} and {max}")));
    }
    Ok(val)
}

fn validate_url(url: &str) -> Result<()> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(GrokError::ConfigInvalid("GROK_API_URL must be a valid http or https URL".into()));
    }
    Ok(())
}

fn read_persisted_config() -> PersistedConfig {
    let path = Config::config_file_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_config_atomic(cfg: &PersistedConfig) -> Result<()> {
    let path = Config::config_file_path();
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    std::fs::create_dir_all(&dir).map_err(|e| GrokError::ConfigFile {
        path: dir.clone(),
        message: e.to_string(),
    })?;

    let tmp = dir.join(format!(".config.tmp.{}", std::process::id()));
    let data = serde_json::to_string_pretty(cfg)?;

    std::fs::write(&tmp, format!("{data}\n")).map_err(|e| GrokError::ConfigFile {
        path: tmp.clone(),
        message: e.to_string(),
    })?;

    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        GrokError::ConfigFile { path, message: e.to_string() }
    })
}

fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.trim().chars().collect();
    if chars.len() <= 8 {
        return "********".into();
    }
    let first: String = chars[..4].iter().collect();
    let last: String = chars[chars.len()-4..].iter().collect();
    format!("{first}********{last}")
}
