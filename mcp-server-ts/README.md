# Tauri MCP Server

A Model Context Protocol (MCP) server that enables AI agents to interact with Tauri desktop applications through standardized interfaces.

## Features

- **Screenshot Capture**: Take screenshots of Tauri windows
- **DOM Access**: Retrieve HTML content from webview windows  
- **Mouse Control**: Simulate mouse clicks, movements, and scrolling
- **Text Input**: Send text input to focused elements
- **JavaScript Execution**: Run arbitrary JavaScript in application context
- **Local Storage Management**: Get, set, remove, and clear localStorage entries
- **Window Management**: Control window position, size, focus, and state

## Installation

```bash
npm install -g tauri-mcp-server
```

## Usage

### With Claude Desktop

Add to your Claude Desktop MCP settings:

```json
{
  "mcpServers": {
    "tauri-mcp": {
      "command": "tauri-mcp-server"
    }
  }
}
```

### With Cursor

Add to your Cursor MCP configuration:

```json
{
  "mcpServers": {
    "tauri-mcp": {
      "command": "npx",
      "args": ["tauri-mcp-server"]
    }
  }
}
```

## Available Tools

- `take_screenshot` - Capture window screenshots
- `get_dom` - Retrieve DOM content from webviews
- `mouse_movement` - Control mouse cursor and clicks
- `text_input` - Send text to focused elements
- `execute_js` - Run JavaScript in application context
- `manage_local_storage` - Handle localStorage operations
- `manage_window` - Control window properties
- `get_element_position` - Find element positions for interaction
- `send_text_to_element` - Send text to specific elements

## Requirements

- Node.js 18+
- A Tauri application with the MCP plugin installed

## Architecture

The server communicates with Tauri applications through:
- IPC (Unix sockets/named pipes) for local communication
- TCP sockets for network communication
- Direct Tauri API integration

## License

MIT - See [LICENSE](../LICENSE) for details.

## Repository

https://github.com/delorenj/tauri-plugin-mcp
