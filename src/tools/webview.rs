use serde::Deserialize; // Add Deserialize for parsing payload
use serde_json::Value;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Listener, Runtime};

use crate::tools::eval::{eval_string, resolve_webview, window_label_from_payload};

/// The DOM read is a single synchronous property access, so it either answers
/// immediately or the page's main thread is wedged - a long timeout would only
/// hold the socket open for longer.
const GET_DOM_TIMEOUT: Duration = Duration::from_secs(5);

// Handler function for the getDom command, following the take_screenshot pattern
pub async fn handle_get_dom<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<crate::socket_server::SocketResponse, crate::error::Error> {
    // Accepts a bare string (what the MCP server has always sent), an object
    // with `window_label`, or nothing at all - which still means "main", so
    // callers that predate the label keep working.
    let window_label = window_label_from_payload(&payload);
    let webview = resolve_webview(app, &window_label)?;

    // Read the DOM by evaluating in the target webview rather than by asking the
    // guest to answer a `got-dom-content` event: the event round trip needed the
    // app to register a listener, and a child webview (the in-app browser) has no
    // guest bundle to register one.
    let result = eval_string(
        &webview,
        "document.documentElement ? document.documentElement.outerHTML : ''",
        GET_DOM_TIMEOUT,
    );

    match result {
        Ok(dom_text) => {
            let data = serde_json::to_value(dom_text).map_err(|e| {
                crate::error::Error::Anyhow(format!("Failed to serialize response: {}", e))
            })?;
            Ok(crate::socket_server::SocketResponse {
                success: true,
                data: Some(data),
                error: None,
            })
        }
        Err(e) => Ok(crate::socket_server::SocketResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
use tauri::Emitter;

// Define the structure for get_element_position payload
#[derive(Debug, Deserialize)]
struct GetElementPositionPayload {
    window_label: String,
    selector_type: String,
    selector_value: String,
    #[serde(default)]
    should_click: bool,
    #[serde(default)]
    raw_coordinates: bool,
}

// Handle getting element position
pub async fn handle_get_element_position<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<crate::socket_server::SocketResponse, crate::error::Error> {
    // Parse the payload
    let payload = serde_json::from_value::<GetElementPositionPayload>(payload).map_err(|e| {
        crate::error::Error::Anyhow(format!("Invalid payload for get_element_position: {}", e))
    })?;

    // Create a channel to receive the result
    let (tx, rx) = mpsc::channel();

    // Event name for the response
    let event_name = "get-element-position-response";

    // Set up the listener for the response
    app.once(event_name, move |event| {
        let payload = event.payload().to_string();
        let _ = tx.send(payload);
    });

    // Prepare the request payload with selector information
    let js_payload = serde_json::json!({
        "windowLabel": payload.window_label,
        "selectorType": payload.selector_type,
        "selectorValue": payload.selector_value,
        "shouldClick": payload.should_click,
        "rawCoordinates": payload.raw_coordinates
    });

    // Emit the event to the webview
    app.emit_to(&payload.window_label, "get-element-position", js_payload)
        .map_err(|e| {
            crate::error::Error::Anyhow(format!("Failed to emit get-element-position event: {}", e))
        })?;

    // Wait for the response with a timeout
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(result) => {
            // Parse the result
            let result_value: Value = serde_json::from_str(&result).map_err(|e| {
                crate::error::Error::Anyhow(format!("Failed to parse result: {}", e))
            })?;

            let success = result_value
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if success {
                Ok(crate::socket_server::SocketResponse {
                    success: true,
                    data: Some(result_value.get("data").cloned().unwrap_or(Value::Null)),
                    error: None,
                })
            } else {
                let error = result_value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error occurred");

                Ok(crate::socket_server::SocketResponse {
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                })
            }
        }
        Err(e) => Ok(crate::socket_server::SocketResponse {
            success: false,
            data: None,
            error: Some(format!(
                "Timeout waiting for element position result: {}",
                e
            )),
        }),
    }
}

// Define the structure for send_text_to_element payload
#[derive(Debug, Deserialize)]
struct SendTextToElementPayload {
    window_label: String,
    selector_type: String,
    selector_value: String,
    text: String,
    #[serde(default = "default_delay_ms")]
    delay_ms: u32,
}

// Default delay_ms value
fn default_delay_ms() -> u32 {
    20
}

// Handle sending text to an element
pub async fn handle_send_text_to_element<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<crate::socket_server::SocketResponse, crate::error::Error> {
    // Parse the payload
    let payload = serde_json::from_value::<SendTextToElementPayload>(payload).map_err(|e| {
        crate::error::Error::Anyhow(format!("Invalid payload for send_text_to_element: {}", e))
    })?;


    // Create a channel to receive the result
    let (tx, rx) = mpsc::channel();

    // Event name for the response
    let event_name = "send-text-to-element-response";

    // Set up the listener for the response
    app.once(event_name, move |event| {
        let payload = event.payload().to_string();
        let _ = tx.send(payload);
    });

    // Prepare the request payload
    let js_payload = serde_json::json!({
        "selectorType": payload.selector_type,
        "selectorValue": payload.selector_value,
        "text": payload.text,
        "delayMs": payload.delay_ms
    });

    // Emit the event to the webview
    app.emit_to(&payload.window_label, "send-text-to-element", js_payload)
        .map_err(|e| {
            crate::error::Error::Anyhow(format!("Failed to emit send-text-to-element event: {}", e))
        })?;

    // Wait for the response with a timeout
    match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        // Longer timeout for typing text
        Ok(result) => {
            // Parse the result
            let result_value: Value = serde_json::from_str(&result).map_err(|e| {
                crate::error::Error::Anyhow(format!("Failed to parse result: {}", e))
            })?;

            let success = result_value
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if success {
                Ok(crate::socket_server::SocketResponse {
                    success: true,
                    data: Some(result_value.get("data").cloned().unwrap_or(Value::Null)),
                    error: None,
                })
            } else {
                let error = result_value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error occurred");

                Ok(crate::socket_server::SocketResponse {
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                })
            }
        }
        Err(e) => Ok(crate::socket_server::SocketResponse {
            success: false,
            data: None,
            error: Some(format!("Timeout waiting for text input completion: {}", e)),
        }),
    }
}
