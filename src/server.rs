use crate::config::Config;
use crate::grok::GrokClient;
use crate::tools::{GetConfigInfoParams, SwitchModelParams, ToggleBuiltinToolsParams, WebFetchParams, WebSearchParams};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};

#[derive(Clone)]
pub struct GrokSearchServer {
    pub config: Config,
    pub client: GrokClient,
}

impl GrokSearchServer {
    pub fn new(config: Config) -> Self {
        let client = GrokClient::new(&config);
        Self { config, client }
    }
}

#[tool_router]
impl GrokSearchServer {
    #[tool(description = r#"
    Performs a third-party web search based on the given query and returns the results
    as a JSON string.

    The `query` should be a clear, self-contained natural-language search query.
    When helpful, include constraints such as topic, time range, language, or domain.

    The `platform` should be the platforms which you should focus on searching, such as "Twitter", "GitHub", "Reddit", etc.

    The `min_results` and `max_results` should be the minimum and maximum number of results to return.
    "#)]
    pub async fn web_search(&self, Parameters(params): Parameters<WebSearchParams>) -> Result<String, McpError> {
        params.validate().map_err(|msg| McpError::invalid_params(msg, None))?;
        self.client.search(params.query.trim(), params.platform.trim(), params.min_results, params.max_results)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    #[tool(description = r#"
    Fetches and extracts the complete content from a specified URL and returns it
    as a structured Markdown document.
    The `url` should be a valid HTTP/HTTPS web address pointing to the target page.
    Ensure the URL is complete and accessible (not behind authentication or paywalls).
    The function will:
    - Retrieve the full HTML content from the URL
    - Parse and extract all meaningful content (text, images, links, tables, code blocks)
    - Convert the HTML structure to well-formatted Markdown
    - Preserve the original content hierarchy and formatting
    - Remove scripts, styles, and other non-content elements
    Returns
    -------
    str
        A Markdown-formatted string containing:
        - Metadata header (source URL, title, fetch timestamp)
        - Table of Contents (if applicable)
        - Complete page content with preserved structure
    "#)]
    pub async fn web_fetch(&self, Parameters(params): Parameters<WebFetchParams>) -> Result<String, McpError> {
        params.validate().map_err(|msg| McpError::invalid_params(msg, None))?;
        self.client.fetch(params.url.trim())
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    #[tool(description = r#"
    Returns the current Grok Search MCP server configuration information and tests the connection.

    This tool is useful for:
    - Verifying that environment variables are correctly configured
    - Testing API connectivity by sending a request to /models endpoint
    - Debugging configuration issues
    - Checking the current API endpoint and settings

    Returns
    -------
    str
        A JSON-encoded string containing configuration details:
        - `api_url`: The configured Grok API endpoint
        - `api_key`: The API key (masked for security, showing only first and last 4 characters)
        - `model`: The currently selected model for search and fetch operations
        - `debug_enabled`: Whether debug mode is enabled
        - `log_level`: Current logging level
        - `log_dir`: Directory where logs are stored
        - `config_status`: Overall configuration status (✅ complete or ❌ error)
        - `connection_test`: Result of testing API connectivity to /models endpoint
          - `status`: Connection status
          - `message`: Status message with model count
          - `response_time_ms`: API response time in milliseconds
    "#)]
    pub async fn get_config_info(&self, _params: Parameters<GetConfigInfoParams>) -> Result<String, McpError> {
        let config_status = "✅ 配置完整".to_string();
        let connection_test = self.client.test_connection().await;

        let payload = serde_json::json!({
            "api_url": &self.config.api_url,
            "api_key": self.config.mask_api_key(),
            "model": &self.config.model,
            "debug_enabled": self.config.debug_enabled,
            "log_level": &self.config.log_level,
            "log_dir": self.config.log_dir.clone().unwrap_or_default(),
            "config_file": Config::config_file_path().to_string_lossy(),
            "config_status": config_status,
            "connection_test": connection_test,
        });

        serde_json::to_string_pretty(&payload).map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    #[tool(description = r#"
    Switches the default Grok model used for search and fetch operations, and persists the setting.

    This tool is useful for:
    - Changing the AI model used for web search and content fetching
    - Testing different models for performance or quality comparison
    - Persisting model preference across sessions

    Parameters
    ----------
    model : str
        The model ID to switch to (e.g., "grok-4-fast", "grok-2-latest", "grok-vision-beta")

    Returns
    -------
    str
        A JSON-encoded string containing:
        - `status`: Success or error status
        - `previous_model`: The model that was being used before
        - `current_model`: The newly selected model
        - `message`: Status message
        - `config_file`: Path where the model preference is saved
    "#)]
    pub async fn switch_model(&self, Parameters(params): Parameters<SwitchModelParams>) -> Result<String, McpError> {
        params.validate().map_err(|msg| McpError::invalid_params(msg, None))?;

        let previous = self.config.model.clone();
        let next = params.model.trim().to_string();

        let payload = match Config::save_model(&next) {
            Ok(()) => serde_json::json!({
                "status": "✅ 成功",
                "previous_model": previous,
                "current_model": next,
                "message": format!("模型已从 {} 切换到 {}", previous, next),
                "config_file": Config::config_file_path().to_string_lossy(),
            }),
            Err(e) => serde_json::json!({
                "status": "❌ 失败",
                "message": format!("切换模型失败: {}", e),
            }),
        };

        serde_json::to_string_pretty(&payload).map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    #[tool(description = r#"
    Toggle Claude Code's built-in WebSearch and WebFetch tools on/off.

    Parameters: action - "on" (block built-in), "off" (allow built-in), "status" (check)
    Returns: JSON with current status and deny list
    "#)]
    pub async fn toggle_builtin_tools(&self, Parameters(params): Parameters<ToggleBuiltinToolsParams>) -> Result<String, McpError> {
        params.validate().map_err(|msg| McpError::invalid_params(msg, None))?;

        let action = params.action.trim().to_lowercase();
        let tools = ["WebFetch", "WebSearch"];
        let mut blocked = self.config.builtin_tools_disabled;

        let message = match action.as_str() {
            "on" => {
                blocked = true;
                Config::save_builtin_tools_disabled(true).ok();
                "官方工具已禁用"
            }
            "off" => {
                blocked = false;
                Config::save_builtin_tools_disabled(false).ok();
                "官方工具已启用"
            }
            _ => if blocked { "官方工具当前已禁用" } else { "官方工具当前已启用" }
        };

        let payload = serde_json::json!({
            "blocked": blocked,
            "deny_list": if blocked { tools.to_vec() } else { vec![] },
            "file": Config::config_file_path().to_string_lossy(),
            "message": message,
        });

        serde_json::to_string_pretty(&payload).map_err(|e| McpError::internal_error(e.to_string(), None))
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for GrokSearchServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "grok-search".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
