//! `execute_js` - run JavaScript in a webview and get the value back.
//!
//! This used to emit a `execute-js` tauri event and block on a matching
//! `execute-js-response`, which meant it only worked if the *guest* had called
//! `setupPluginListeners()` from `guest-js`. Host apps that register their own
//! listeners (diy-platform does) never wired that one up, so nothing ever
//! answered and every call died on the 5s timeout. The guest-side implementation
//! also compiled the caller's code with `new Function`, which a real CSP
//! (`script-src 'self'`) blocks outright. Evaluating from the Rust side needs no
//! guest cooperation and works for child webviews too.

use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Runtime};

use crate::error::Error;
use crate::socket_server::SocketResponse;
use crate::tools::eval::{eval_json, resolve_webview, DEFAULT_WINDOW_LABEL};

const DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// A script that has not answered in a minute never will, and every pending call
/// holds a socket thread open, so the caller's timeout is capped rather than
/// trusted.
const MAX_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteJsRequest {
    code: String,
    window_label: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExecuteJsResponse {
    /// The evaluated value as JSON - an object stays an object rather than being
    /// flattened to a display string.
    result: Value,
    /// The JS `typeof` of the value, so a caller can tell `null` from the string
    /// `"null"` and from a script that returned nothing.
    #[serde(rename = "type")]
    result_type: String,
}

pub async fn handle_execute_js<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<SocketResponse, Error> {
    let request: ExecuteJsRequest = serde_json::from_value(payload)
        .map_err(|e| Error::Anyhow(format!("Invalid payload for execute_js: {}", e)))?;

    if request.code.trim().is_empty() {
        return Ok(SocketResponse {
            success: false,
            data: None,
            error: Some("execute_js requires a non-empty `code`".to_string()),
        });
    }

    let window_label = request
        .window_label
        .as_deref()
        .filter(|label| !label.is_empty())
        .unwrap_or(DEFAULT_WINDOW_LABEL);
    let webview = resolve_webview(app, window_label)?;

    let timeout = Duration::from_millis(
        request
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(1, MAX_TIMEOUT_MS),
    );

    match eval_json(&webview, &request.code, timeout) {
        Ok(outcome) => {
            let data = serde_json::to_value(ExecuteJsResponse {
                result: outcome.value,
                result_type: outcome.type_name,
            })
            .map_err(|e| Error::Anyhow(format!("Failed to serialize response: {}", e)))?;

            Ok(SocketResponse {
                success: true,
                data: Some(data),
                error: None,
            })
        }
        // A thrown exception is the caller's problem, not a transport failure, so
        // it surfaces as an error string instead of being swallowed into a null.
        Err(e) => Ok(SocketResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
