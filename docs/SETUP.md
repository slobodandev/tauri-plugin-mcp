# Setup Guide

This document provides detailed setup instructions for the Tauri MCP Plugin.

## Building the Plugin

```bash
bun install
bun run build && bun run build-plugin
```

## Integration with Tauri Application

### 1. Add Dependency to Cargo.toml

In your Tauri app's `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-mcp = { path = "../../tauri-plugin-mcp" }
```

### 2. Add to package.json

```json
{
  "dependencies": {
    "tauri-plugin-mcp": "file:../tauri-plugin-mcp"
  }
}
```

### 3. Register Plugin (Development Only)

⚠️ **Important:** Set the application name correctly - this is used to identify windows for screenshots.

```rust
#[cfg(debug_assertions)]
{
    use log::info;
    info!("Development build detected, enabling MCP plugin");

    tauri::Builder::default()
        .plugin(tauri_mcp::init_with_config(
            tauri_mcp::PluginConfig::new("YOUR_APP_NAME".to_string())
                .start_socket_server(true)
                // Choose IPC (default) or TCP mode:
                .socket_path("/tmp/tauri-mcp.sock")  // IPC
                // .tcp("127.0.0.1", 9999)            // TCP
        ))
        // ... rest of your builder config
}
```

## Building the MCP Server

```bash
cd mcp-server-ts
bun install
bun run build
```

## Configuration

See [MCP_SERVER.md](./MCP_SERVER.md) for detailed MCP server configuration and [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) for common issues.
