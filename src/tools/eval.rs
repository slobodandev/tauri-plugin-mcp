//! Webview lookup and JavaScript evaluation, shared by every tool that has to
//! reach into a page.
//!
//! Both halves live here because four call sites used to hand-roll them:
//!
//! * **Label resolution.** Every tool re-derived `window_label` and then called
//!   `get_webview_window`, which only ever matches a *top level* webview window.
//!   Child webviews (`Window::add_child`, which is how the in-app browser is
//!   built) are registered in the same manager map but are not webview windows,
//!   so they were unreachable by label. `Manager::get_webview` matches both,
//!   which is why the crate now asks tauri for the `unstable` feature.
//!
//! * **Getting a value back out of JS.** `Webview::eval` is fire-and-forget;
//!   `Webview::eval_with_callback` (tauri 2.11.5, `src/webview/mod.rs:1929`) hands
//!   the evaluated value to a callback on the event loop thread. We bridge that
//!   to the caller with a channel and a deadline so a script that never settles
//!   fails cleanly instead of pinning the socket open.
//!
//! Two platform facts shape the JS we send:
//!
//! 1. Exceptions are lost. Upstream says so on `eval_with_callback` ("Exception
//!    is ignored because of the limitation on Windows"), and on WebKitGTK a
//!    failed script reaches the callback as an empty string
//!    (`wry-0.55.1/src/webkitgtk/mod.rs:706`, where the error is
//!    `unwrap_or_default`ed). So we never trust the raw completion value: the
//!    script we send always returns an envelope *we* built inside a `try`.
//! 2. No `eval`/`new Function`. Host apps ship a real CSP (diy-platform uses
//!    `script-src 'self'` with no `'unsafe-eval'`), so the caller's code has to
//!    be embedded literally in the script we build, not compiled at runtime.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, Webview};

use crate::error::Error;

/// The webview every tool falls back to when the caller sends no label.
pub const DEFAULT_WINDOW_LABEL: &str = "main";

/// Gap between polls while waiting on a script that returned a promise.
const PROMISE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Suffix for the global we park a pending promise result in, bumped per call so
/// two concurrent socket clients cannot read each other's slot.
static SLOT_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum EvalError {
    /// The script never reached the webview at all.
    Transport(String),
    /// The page threw. The text is the JS stack/message verbatim.
    Script(String),
    Timeout(String),
    /// The webview accepted the script but handed back nothing readable.
    NoResult(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Transport(s) => write!(f, "JavaScript execution error: {}", s),
            EvalError::Script(s) => write!(f, "JavaScript error: {}", s),
            EvalError::Timeout(s) => write!(f, "Operation timed out: {}", s),
            EvalError::NoResult(s) => write!(f, "No result from webview: {}", s),
        }
    }
}

impl From<EvalError> for Error {
    fn from(err: EvalError) -> Self {
        Error::Anyhow(err.to_string())
    }
}

/// A successfully evaluated script.
pub struct EvalOutcome {
    /// The value, as JSON. `undefined` arrives as `null` (see `type_name`).
    pub value: Value,
    /// The JS `typeof` of the value before encoding, with `null` reported as
    /// `"null"` rather than `typeof`'s historical `"object"`.
    pub type_name: String,
}

/// Pull the target label out of a socket payload.
///
/// Three shapes are accepted because callers already send all three: a bare
/// string (`get_dom` has always sent one), an object carrying `window_label`,
/// and nothing at all. Anything unrecognised falls back to `main`, so callers
/// that predate the field keep working.
pub fn window_label_from_payload(payload: &Value) -> String {
    if let Value::String(label) = payload {
        if !label.is_empty() {
            return label.clone();
        }
    }
    payload
        .get("window_label")
        .and_then(Value::as_str)
        .filter(|label| !label.is_empty())
        .unwrap_or(DEFAULT_WINDOW_LABEL)
        .to_string()
}

/// Resolve a label to a webview, child webviews included.
pub fn resolve_webview<R: Runtime>(app: &AppHandle<R>, label: &str) -> Result<Webview<R>, Error> {
    app.get_webview(label).ok_or_else(|| {
        // Listing what *is* there turns "not found" from a dead end into a hint,
        // which matters most for child webviews whose labels are generated.
        let mut known: Vec<String> = app.webviews().keys().cloned().collect();
        known.sort();
        Error::WindowNotFound(format!("{} (known webviews: {})", label, known.join(", ")))
    })
}

/// Evaluate `code` in `webview` and return its value.
///
/// The caller's code may be a bare expression (`document.title`) or a statement
/// list with an explicit `return`; both work, and `await` is allowed in the
/// latter. A returned promise is awaited up to the same deadline.
pub fn eval_json<R: Runtime>(
    webview: &Webview<R>,
    code: &str,
    timeout: Duration,
) -> Result<EvalOutcome, EvalError> {
    let deadline = Instant::now() + timeout;
    let slot = js_string(&format!(
        "__tauriMcpEval_{}",
        SLOT_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    // Expression form first: it is what callers almost always send and the only
    // form that yields a value without an explicit `return`. If the code is a
    // statement list the whole script is a syntax error, which the platform
    // reports by handing the callback nothing - that is our cue to retry as
    // statements. Nothing ran, so the retry cannot double up side effects.
    //
    // The trailing semicolon has to go: `document.title;` is a natural thing to
    // send, and it only becomes a syntax error because of the `return (...)` we
    // wrap it in. The statement form below keeps the code verbatim.
    let expression_code = code.trim_end_matches(|c: char| c == ';' || c.is_whitespace());
    let expression = build_script(
        &slot,
        &format!("(function(){{return (\n{}\n);}})()", expression_code),
    );
    let mut envelope = decode_envelope(&eval_raw(webview, &expression, deadline)?);

    if envelope.is_none() {
        // The statement wrapper is `async` so that top-level `await` parses. It
        // therefore always returns a promise and always takes the polling path.
        let statements = build_script(&slot, &format!("(async function(){{\n{}\n}})()", code));
        envelope = decode_envelope(&eval_raw(webview, &statements, deadline)?);
    }

    let mut envelope = envelope.ok_or_else(|| {
        EvalError::NoResult(
            "the webview ran the script but returned nothing - the code is most likely a syntax \
             error"
                .to_string(),
        )
    })?;

    while envelope.pending {
        if Instant::now() >= deadline {
            // Nobody will ever read the parked slot now, so drop it rather than
            // leak a global into the page for the rest of its life.
            let _ = webview.eval(format!(
                "(function(){{try{{delete globalThis[{}];}}catch(e){{}}}})()",
                slot
            ));
            return Err(EvalError::Timeout(format!(
                "the script returned a promise that did not settle within {}ms",
                timeout.as_millis()
            )));
        }
        std::thread::sleep(PROMISE_POLL_INTERVAL);
        let poll = POLL_TEMPLATE.replace("%SLOT%", &slot);
        envelope = decode_envelope(&eval_raw(webview, &poll, deadline)?).ok_or_else(|| {
            EvalError::NoResult("lost track of the pending result while polling".to_string())
        })?;
    }

    if envelope.ok {
        Ok(EvalOutcome {
            value: envelope.value,
            type_name: envelope.type_name,
        })
    } else {
        Err(EvalError::Script(envelope.error))
    }
}

/// Evaluate `code` and require a string back - the shape most tools want.
pub fn eval_string<R: Runtime>(
    webview: &Webview<R>,
    code: &str,
    timeout: Duration,
) -> Result<String, EvalError> {
    let outcome = eval_json(webview, code, timeout)?;
    match outcome.value {
        Value::String(text) => Ok(text),
        other => Err(EvalError::Script(format!(
            "expected a string but the page returned {} ({})",
            other, outcome.type_name
        ))),
    }
}

/// One round trip: hand the script to the webview, wait for the callback.
fn eval_raw<R: Runtime>(
    webview: &Webview<R>,
    script: &str,
    deadline: Instant,
) -> Result<String, EvalError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(EvalError::Timeout(
            "ran out of time before the script could be sent".to_string(),
        ));
    }

    let (tx, rx) = mpsc::channel();
    webview
        .eval_with_callback(script.to_string(), move |raw| {
            // The receiver is gone once we time out; dropping the value is right.
            let _ = tx.send(raw);
        })
        .map_err(|e| EvalError::Transport(format!("could not reach the webview: {}", e)))?;

    // Blocking is safe here: each socket connection gets its own thread and its
    // own runtime (see socket_server::handle_client), and the callback is
    // delivered on the tauri event loop thread, not this one.
    rx.recv_timeout(remaining).map_err(|_| {
        EvalError::Timeout(format!(
            "the webview did not answer within {}ms",
            remaining.as_millis()
        ))
    })
}

struct Envelope {
    pending: bool,
    ok: bool,
    value: Value,
    type_name: String,
    error: String,
}

/// Read the envelope our script returns. `None` means the script did not run -
/// a syntax error, or a platform that swallowed the failure.
fn decode_envelope(raw: &str) -> Option<Envelope> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "undefined" {
        return None;
    }

    // WebKitGTK and WebView2 hand back the JSON *encoding* of the completion
    // value, so our envelope - itself a string - arrives double encoded. Accept
    // an already-decoded object too, in case a runtime skips that step.
    let decoded = match serde_json::from_str::<Value>(trimmed).ok()? {
        Value::String(inner) => serde_json::from_str::<Value>(&inner).ok()?,
        other => other,
    };

    match decoded.get("s").and_then(Value::as_str)? {
        "pending" => Some(Envelope {
            pending: true,
            ok: false,
            value: Value::Null,
            type_name: String::new(),
            error: String::new(),
        }),
        "done" => Some(Envelope {
            pending: false,
            ok: decoded.get("ok").and_then(Value::as_bool).unwrap_or(false),
            value: decoded.get("v").cloned().unwrap_or(Value::Null),
            type_name: decoded
                .get("t")
                .and_then(Value::as_str)
                .unwrap_or("undefined")
                .to_string(),
            error: decoded
                .get("e")
                .and_then(Value::as_str)
                .unwrap_or("the page reported an error without a message")
                .to_string(),
        }),
        _ => None,
    }
}

fn build_script(slot: &str, inner: &str) -> String {
    // `%SLOT%` is substituted first so that caller code containing the literal
    // `%SLOT%` cannot be rescanned; `str::replace` never re-reads what it wrote.
    EVAL_TEMPLATE.replace("%SLOT%", slot).replace("%INNER%", inner)
}

/// Quote a Rust string as a JS string literal.
fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

const EVAL_TEMPLATE: &str = r#"(function(){
var __k=%SLOT%;
var __enc=function(ok,v,e){try{return ok?JSON.stringify({s:"done",ok:true,t:(v===null?"null":typeof v),v:(v===undefined?null:v)}):JSON.stringify({s:"done",ok:false,e:String((e&&(e.stack||e.message))||e)});}catch(x){return JSON.stringify({s:"done",ok:false,e:"result is not JSON-serializable: "+String(x)});}};
var __park=function(p){globalThis[__k]={pending:true,result:null};p.then(function(v){globalThis[__k]={pending:false,result:__enc(true,v,null)};},function(e){globalThis[__k]={pending:false,result:__enc(false,null,e)};});return JSON.stringify({s:"pending"});};
try{var __r=%INNER%;return (__r&&typeof __r.then==="function")?__park(__r):__enc(true,__r,null);}catch(__e){return __enc(false,null,__e);}
})()"#;

const POLL_TEMPLATE: &str = r#"(function(){var __k=%SLOT%;var s=globalThis[__k];if(!s){return JSON.stringify({s:"done",ok:false,e:"the pending result slot disappeared - the page probably navigated"});}if(s.pending){return JSON.stringify({s:"pending"});}delete globalThis[__k];return s.result;})()"#;
