# Grok Search MCP (Rust)

A Rust implementation of the MCP (Model Context Protocol) server for Grok-powered web search.

## Features

- **5 MCP Tools**: web_search, web_fetch, get_config_info, switch_model, toggle_builtin_tools
- **Single Binary**: Zero runtime dependencies, cross-platform support
- **Streaming**: SSE response parsing with retry mechanism
- **Configuration**: Environment variables + JSON file persistence

## Installation

### One-line Install (Recommended)

```bash
npx grok-search-mcp-rust
```

This will:
1. Detect your platform automatically
2. Download the appropriate binary
3. Optionally configure Claude Code

### Manual Download

Download from [GitHub Releases](https://github.com/pdxxxx/grok-search-mcp-rust/releases):

| Platform | Binary |
|----------|--------|
| Linux x64 | `grok-search-mcp-linux-amd64` |
| Linux ARM64 | `grok-search-mcp-linux-arm64` |
| macOS x64 | `grok-search-mcp-macos-amd64` |
| macOS ARM64 | `grok-search-mcp-macos-arm64` |
| Windows x64 | `grok-search-mcp-windows-amd64.exe` |

### Build from Source

```bash
git clone https://github.com/pdxxxx/grok-search-mcp-rust.git
cd grok-search-mcp-rust
cargo build --release
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `GROK_API_URL` | Yes | - | Grok API endpoint |
| `GROK_API_KEY` | Yes | - | API authentication key |
| `GROK_MODEL` | No | `grok-4-fast` | Default model |
| `GROK_RETRY_MAX_ATTEMPTS` | No | `3` | Max retry attempts (1-10) |
| `GROK_RETRY_MULTIPLIER` | No | `1.0` | Backoff multiplier |
| `GROK_RETRY_MAX_WAIT` | No | `10` | Max wait seconds |

### Claude Code Integration

```bash
claude mcp add-json grok-search --scope user '{
  "type": "stdio",
  "command": "/path/to/grok-search-mcp",
  "env": {
    "GROK_API_URL": "https://api.x.ai/v1",
    "GROK_API_KEY": "your-api-key"
  }
}'
```

## Tools

### web_search

Search the web using Grok API.

```json
{
  "query": "MCP protocol documentation",
  "platform": "github",
  "min_results": 3,
  "max_results": 10
}
```

### web_fetch

Fetch and convert web page to Markdown.

```json
{
  "url": "https://example.com"
}
```

### get_config_info

Get current configuration and test API connection.

### switch_model

Switch the Grok model (persisted to config file).

```json
{
  "model": "grok-2-latest"
}
```

### toggle_builtin_tools

Toggle Claude's built-in WebSearch/WebFetch tools.

```json
{
  "action": "on"
}
```

## License

MIT
