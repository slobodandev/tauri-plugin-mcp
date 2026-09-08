# Tauri MCP Server

This is a Model Context Protocol (MCP) server that connects to a Tauri application's socket server to provide tools for controlling and interacting with the Tauri application.

## Overview

The server bridges MCP clients (like LLMs) with a Tauri application by:

1. Connecting to the Tauri socket server via a Unix socket/named pipe
2. Providing MCP tools that map to Tauri functionality
3. Running as a stdio-based MCP server that any MCP client can use

## Available Tools

The server provides the following MCP tools. Every `window_label` is resolved
against the app's whole webview registry, so a *child* webview created with
`Window::add_child` (an in-app browser tab, label `browser::<id>`) is addressable
by label exactly like a top-level window.

### `take_screenshot`

Take a screenshot of a Tauri application window.

**Parameters:**
- `window_label` (optional): The label of the window to capture (default: "main")
- `quality` (optional): JPEG quality from 1-100
- `max_width` (optional): Maximum image width in pixels
- `max_size_mb` (optional): Maximum file size in MB

**Returns:**
- Image content with base64-encoded data

### `execute_js`

Evaluate JavaScript in a webview and get the value back.

**Parameters:**
- `code`: JavaScript to evaluate. A bare expression (`document.title`) returns its
  value; several statements need an explicit `return` and may use `await`. A
  returned promise is awaited up to the timeout.
- `window_label` (optional): The webview to evaluate in (default: "main")
- `timeout_ms` (optional): Maximum execution time in milliseconds (default 5000,
  capped at 60000)

**Returns:**
- `result`: the evaluated value as JSON (`undefined` comes back as `null`)
- `type`: the JS `typeof` of the value, with `null` reported as `"null"`

A thrown exception is returned as an error, not as a null result.

### `get_dom`

Get the HTML DOM content of a webview.

**Parameters:**
- `window_label` (optional): The webview to read (default: "main")

**Returns:**
- HTML content as a string

### `load_uri`

Navigate a webview to a URL.

**Parameters:**
- `url`: an `http:` or `https:` URL. Every other scheme (`file:`, `data:`,
  `javascript:`) is rejected — the socket is an unauthenticated automation
  surface, and an unchecked navigate would be a local-file read primitive.
- `window_label` (optional): The webview to navigate (default: "main")

**Returns:**
- `window_label` and the normalized `url`

### `go_back` / `go_forward`

Move a webview through its session history.

**Parameters:**
- `window_label` (optional): The webview to navigate (default: "main")

**Returns:**
- `history_length`: `history.length` as seen just before the move. The webview
  exposes no history API, so these are driven through the page's own `history`
  object and cannot report whether an entry existed — a length of 1 means the
  call was a no-op.

### `get_url`

Read the URL a webview is currently showing.

**Parameters:**
- `window_label` (optional): The webview to read (default: "main")

**Returns:**
- `window_label` and the current `url`

### `manage_window`

Control Tauri application windows.

**Parameters:**
- `operation`: Operation to perform (e.g., "focus", "minimize", "maximize", "setPosition", "setSize")
- `window_label` (optional): Target window (default: "main")
- `x` (optional): X position for setPosition
- `y` (optional): Y position for setPosition
- `width` (optional): Width for setSize
- `height` (optional): Height for setSize

**Returns:**
- Success message

### `manage_local_storage`

Manage localStorage in the Tauri webview.

**Parameters:**
- `action`: Action to perform ("get", "set", "remove", "clear", or "keys")
- `key` (optional): Key to get, set, or remove
- `value` (optional): Value to set
- `window_label` (optional): Target window (default: "main")

**Returns:**
- Operation result

## Setup and Usage

1. Ensure the Tauri application is running with the socket server active
2. Start this MCP server
3. Connect your MCP client to this server
4. Use the tools to interact with the Tauri application

The server connects to the Tauri socket at `/private/tmp/tauri-mcp.sock`.

## Error Handling

All tools follow the MCP error reporting convention:
- If a tool succeeds, it returns a result object with `content`
- If a tool fails, it returns an object with `isError: true` and an error message in `content`

## Example

Using the `take_screenshot` tool from an MCP client:

```json
{
  "name": "take_screenshot",
  "arguments": {
    "window_label": "main",
    "quality": 90,
    "max_width": 1920
  }
}
```

The response will include base64-encoded image data that can be rendered or saved as a JPEG file. 