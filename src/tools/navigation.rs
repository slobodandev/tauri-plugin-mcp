//! Navigation control for webviews, written for the child webviews the host app
//! uses as an in-app browser (`Window::add_child`, labels prefixed `browser::`).
//!
//! These all go through `eval::resolve_webview`, so they address child webviews
//! and top-level webview windows alike.

use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Runtime, Url};

use crate::error::Error;
use crate::socket_server::SocketResponse;
use crate::tools::eval::{eval_json, resolve_webview, window_label_from_payload};

/// History navigation is a one-line script; if the page cannot run it that fast
/// it is wedged, and waiting longer only holds the socket open.
const HISTORY_TIMEOUT: Duration = Duration::from_secs(5);

/// `Webview` has no back/forward API in tauri 2.11 - only `navigate` and `url` -
/// so history moves are driven through the page's own `history` object. The
/// limitation that comes with that: the webview never tells us whether there was
/// an entry to move to, so we report `history.length` and let the caller judge
/// (a length of 1 means the move was a no-op).
const GO_BACK_JS: &str = "(function(){var n=history.length;history.back();return n;})()";
const GO_FORWARD_JS: &str = "(function(){var n=history.length;history.forward();return n;})()";

#[derive(Debug, serde::Deserialize)]
struct LoadUriRequest {
    window_label: Option<String>,
    url: String,
}

pub async fn handle_load_uri<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<SocketResponse, Error> {
    let request: LoadUriRequest = serde_json::from_value(payload)
        .map_err(|e| Error::Anyhow(format!("Invalid payload for load_uri: {}", e)))?;

    let url = match validate_navigable(&request.url) {
        Ok(url) => url,
        Err(message) => {
            return Ok(SocketResponse {
                success: false,
                data: None,
                error: Some(message),
            })
        }
    };

    let window_label = request
        .window_label
        .as_deref()
        .filter(|label| !label.is_empty())
        .unwrap_or(crate::tools::eval::DEFAULT_WINDOW_LABEL)
        .to_string();
    let webview = resolve_webview(app, &window_label)?;

    webview
        .navigate(url.clone())
        .map_err(|e| Error::WindowOperationFailed(format!("navigate failed: {}", e)))?;

    Ok(SocketResponse {
        success: true,
        data: Some(json!({ "window_label": window_label, "url": url.to_string() })),
        error: None,
    })
}

pub async fn handle_go_back<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<SocketResponse, Error> {
    history_move(app, payload, GO_BACK_JS).await
}

pub async fn handle_go_forward<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<SocketResponse, Error> {
    history_move(app, payload, GO_FORWARD_JS).await
}

pub async fn handle_get_url<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
) -> Result<SocketResponse, Error> {
    let window_label = window_label_from_payload(&payload);
    let webview = resolve_webview(app, &window_label)?;

    match webview.url() {
        Ok(url) => Ok(SocketResponse {
            success: true,
            data: Some(json!({ "window_label": window_label, "url": url.to_string() })),
            error: None,
        }),
        Err(e) => Ok(SocketResponse {
            success: false,
            data: None,
            error: Some(format!("Could not read the webview URL: {}", e)),
        }),
    }
}

async fn history_move<R: Runtime>(
    app: &AppHandle<R>,
    payload: Value,
    script: &str,
) -> Result<SocketResponse, Error> {
    let window_label = window_label_from_payload(&payload);
    let webview = resolve_webview(app, &window_label)?;

    match eval_json(&webview, script, HISTORY_TIMEOUT) {
        Ok(outcome) => Ok(SocketResponse {
            success: true,
            data: Some(json!({ "window_label": window_label, "history_length": outcome.value })),
            error: None,
        }),
        Err(e) => Ok(SocketResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Only http(s) may be navigated to.
///
/// The socket is an unauthenticated automation surface, so an unchecked
/// `navigate` would be a local-file read primitive: `file:` would pull any
/// readable path into a webview that `get_dom` can then dump, and `javascript:`
/// / `data:` would inject script into whatever origin the webview holds.
fn validate_navigable(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim())
        .map_err(|e| format!("`{}` is not a valid URL: {}", raw, e))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "load_uri only navigates to http and https URLs; `{}:` is rejected",
                other
            ))
        }
    }

    // `http:///path` parses but has no host; navigating there is meaningless and
    // the failure would surface much later, inside the webview.
    if url.host_str().unwrap_or_default().is_empty() {
        return Err(format!("`{}` has no host", raw));
    }

    Ok(url)
}
