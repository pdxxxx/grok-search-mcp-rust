use super::prompts::{FETCH_PROMPT, SEARCH_PROMPT};
use crate::config::Config;
use crate::error::{GrokError, Result};
use chrono::Local;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::warn;

const CONNECT_TIMEOUT: u64 = 10;
const READ_TIMEOUT: u64 = 30;
const REQUEST_TIMEOUT: u64 = 120;
const MAX_CONTENT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GrokClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
    retry_max_attempts: u32,
    retry_multiplier: f64,
    retry_max_wait: u64,
}

impl GrokClient {
    pub fn new(config: &Config) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", config.api_key.trim())).unwrap());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(USER_AGENT, HeaderValue::from_str(&format!("grok-search-mcp/{}", env!("CARGO_PKG_VERSION"))).unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            base_url: config.api_url.clone(),
            model: config.model.clone(),
            retry_max_attempts: config.retry_max_attempts,
            retry_multiplier: config.retry_multiplier,
            retry_max_wait: config.retry_max_wait,
        }
    }

    pub async fn search(&self, query: &str, platform: &str, min_results: u32, max_results: u32) -> Result<String> {
        let mut user_content = String::new();
        if needs_time_context(query) {
            user_content.push_str(&time_context());
        }
        user_content.push_str(query);

        if !platform.trim().is_empty() {
            user_content.push_str(&format!(
                "\n\nYou should search the web for the information you need, and focus on these platform: {}",
                platform.trim()
            ));
        }
        if max_results > 0 {
            user_content.push_str(&format!(
                "\n\nYou should return the results in a JSON format, and the results should at least be {} and at most be {} results.",
                min_results, max_results
            ));
        }

        self.chat_stream(&user_content, SEARCH_PROMPT).await
    }

    pub async fn fetch(&self, url: &str) -> Result<String> {
        let user_content = format!("{}\n获取该网页内容并返回其结构化Markdown格式", url.trim());
        self.chat_stream(&user_content, FETCH_PROMPT).await
    }

    pub async fn test_connection(&self) -> ConnectionTestResult {
        let url = format!("{}/models", self.base_url);
        let start = Instant::now();

        match self.client.get(&url).send().await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let status = resp.status();

                if status.is_success() {
                    match resp.json::<serde_json::Value>().await {
                        Ok(v) => ConnectionTestResult {
                            status: "success".into(),
                            response_time_ms: Some(elapsed),
                            model_count: v.get("data").and_then(|d| d.as_array()).map(|a| a.len()),
                            error_code: None,
                            message: Some(format!("OK (HTTP {})", status.as_u16())),
                        },
                        Err(e) => ConnectionTestResult {
                            status: "error".into(),
                            response_time_ms: Some(elapsed),
                            model_count: None,
                            error_code: Some("PARSE_ERROR".into()),
                            message: Some(e.to_string()),
                        },
                    }
                } else {
                    let code = status.as_u16();
                    ConnectionTestResult {
                        status: "error".into(),
                        response_time_ms: Some(elapsed),
                        model_count: None,
                        error_code: Some(classify_status(code)),
                        message: Some(format!("HTTP {}", code)),
                    }
                }
            }
            Err(e) => ConnectionTestResult {
                status: "error".into(),
                response_time_ms: None,
                model_count: None,
                error_code: Some(if e.is_timeout() { "TIMEOUT" } else if e.is_connect() { "CONNECTION_FAILURE" } else { "NETWORK_ERROR" }.into()),
                message: Some(e.to_string()),
            },
        }
    }

    async fn chat_stream(&self, user_content: &str, system_prompt: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);
        let payload = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_content },
            ],
            "stream": true
        });

        let mut last_err = String::new();
        for attempt in 0..=self.retry_max_attempts {
            match self.try_stream_request(&url, &payload).await {
                Ok(content) => return Ok(content),
                Err(e) => {
                    if !is_retryable(&e) || attempt >= self.retry_max_attempts {
                        if attempt >= self.retry_max_attempts {
                            return Err(GrokError::MaxRetries { attempts: self.retry_max_attempts + 1, last_error: e.to_string() });
                        }
                        return Err(e);
                    }
                    last_err = e.to_string();
                    let delay = self.backoff(attempt);
                    warn!("Grok API error, retrying in {:?} (attempt {}/{})", delay, attempt + 1, self.retry_max_attempts + 1);
                    tokio::time::sleep(delay).await;
                }
            }
        }
        Err(GrokError::MaxRetries { attempts: self.retry_max_attempts + 1, last_error: last_err })
    }

    async fn try_stream_request(&self, url: &str, payload: &serde_json::Value) -> Result<String> {
        let mut resp = self.client.post(url).json(payload).send().await.map_err(map_err)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GrokError::Api { status, message: body });
        }

        let mut content = String::new();
        let mut buffer = Vec::new();
        let mut done = false;

        loop {
            let chunk = tokio::time::timeout(Duration::from_secs(READ_TIMEOUT), resp.chunk())
                .await
                .map_err(|_| GrokError::Timeout(READ_TIMEOUT))?
                .map_err(map_err)?;

            let Some(data) = chunk else { break };
            buffer.extend_from_slice(&data);

            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line);
                let line = line.trim();

                if line.is_empty() || line.starts_with(':') { continue; }
                if !line.starts_with("data:") { continue; }

                let data = line[5..].trim();
                if data == "[DONE]" { done = true; break; }
                if data.is_empty() { continue; }

                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                        for choice in choices {
                            if let Some(text) = choice.get("delta").and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                                content.push_str(text);
                                if content.len() > MAX_CONTENT_BYTES {
                                    content.truncate(MAX_CONTENT_BYTES);
                                    warn!("Content exceeded 10MB, truncating");
                                    return Ok(content);
                                }
                            }
                        }
                    }
                }
            }
            if done { break; }
        }

        if !done { warn!("Stream ended without [DONE]"); }
        Ok(content)
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let base = 1.0_f64 * self.retry_multiplier.powi(attempt as i32);
        let capped = base.min(self.retry_max_wait as f64);
        let jitter = rand::thread_rng().gen_range(0.9..=1.1);
        Duration::from_secs_f64((capped * jitter).max(0.1))
    }
}

fn map_err(e: reqwest::Error) -> GrokError {
    if e.is_timeout() { GrokError::Timeout(REQUEST_TIMEOUT) } else { GrokError::Http(e) }
}

fn is_retryable(e: &GrokError) -> bool {
    match e {
        GrokError::Timeout(_) => true,
        GrokError::Http(e) => e.is_timeout() || e.is_connect(),
        GrokError::Api { status, .. } => matches!(status, 429 | 500 | 502 | 503 | 504),
        _ => false,
    }
}

fn classify_status(code: u16) -> String {
    match code {
        401 | 403 => "AUTH_ERROR",
        404 => "NOT_FOUND",
        429 => "RATE_LIMIT",
        500..=599 => "SERVER_ERROR",
        _ => "HTTP_ERROR",
    }.into()
}

fn needs_time_context(query: &str) -> bool {
    let cn = ["今天", "昨天", "明天", "现在", "最新", "最近", "本周", "本月", "今年"];
    for kw in cn { if query.contains(kw) { return true; } }

    let lower = query.to_lowercase();
    let en = ["today", "yesterday", "tomorrow", "now", "latest", "recent", "current", "this week", "this month", "this year"];
    for kw in en { if lower.contains(kw) { return true; } }

    // Check for years 2020-2099
    for word in query.split(|c: char| !c.is_ascii_digit()) {
        if word.len() == 4 {
            if let Ok(year) = word.parse::<u16>() {
                if (2020..=2099).contains(&year) { return true; }
            }
        }
    }
    false
}

fn time_context() -> String {
    let now = Local::now();
    let offset = now.offset().local_minus_utc();
    let sign = if offset >= 0 { '+' } else { '-' };
    let hours = offset.unsigned_abs() / 3600;
    format!("Current time: {} (UTC{}{})\n", now.format("%Y-%m-%d %H:%M:%S"), sign, hours)
}
