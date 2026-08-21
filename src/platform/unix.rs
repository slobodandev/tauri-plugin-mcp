use crate::models::ScreenshotResponse;
use crate::{Error, Result};
use tauri::Runtime;

// Import shared functionality
use crate::desktop::ScreenshotContext;
use crate::platform::shared::handle_screenshot_task;
use crate::shared::ScreenshotParams;

#[cfg(target_os = "linux")]
use crate::desktop::create_success_response;
#[cfg(target_os = "linux")]
use crate::tools::take_screenshot::process_image;
#[cfg(target_os = "linux")]
use log::info;
#[cfg(target_os = "linux")]
use std::sync::mpsc;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};

/// How long the socket-server thread waits for the GTK main loop to deliver the
/// snapshot before giving up. The snapshot itself is near-instant; this only
/// guards against a wedged or never-running main loop.
#[cfg(target_os = "linux")]
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

// Linux implementation: capture the webview in-process via WebKitGTK's own
// snapshot API. This works headlessly-ish (no compositor screen-grab, no
// portal prompt, no X11-only tooling) because WebKitGTK renders the page into
// a cairo surface for us.
//
// Threading: `with_webview` hands us the `webkit2gtk::WebView` on the GTK main
// thread, and `snapshot` is async - its callback also fires on the main loop.
// We are called from a `spawn_blocking` worker, so we hand the bytes back over
// an mpsc channel and block on it with a timeout.
#[cfg(target_os = "linux")]
pub async fn take_screenshot<R: Runtime>(
    params: ScreenshotParams,
    window_context: ScreenshotContext<R>,
) -> Result<ScreenshotResponse> {
    let params_clone = params.clone();
    let window = window_context.window.clone();
    let window_label = params
        .window_label
        .clone()
        .unwrap_or_else(|| "main".to_string());

    handle_screenshot_task(move || {
        let (tx, rx) = mpsc::channel::<std::result::Result<Vec<u8>, String>>();

        info!(
            "[TAURI-MCP] Requesting WebKitGTK snapshot for window '{}'",
            window_label
        );

        window
            .with_webview(move |platform_webview| {
                let webview = platform_webview.inner();
                webview.snapshot(
                    SnapshotRegion::Visible,
                    SnapshotOptions::NONE,
                    None::<&gio::Cancellable>,
                    move |snapshot: std::result::Result<cairo::Surface, glib::Error>| {
                        let payload = match snapshot {
                            Ok(surface) => {
                                let mut png: Vec<u8> = Vec::new();
                                match surface.write_to_png(&mut png) {
                                    Ok(()) if png.is_empty() => Err(
                                        "WebKitGTK returned an empty snapshot surface".to_string(),
                                    ),
                                    Ok(()) => Ok(png),
                                    Err(e) => Err(format!(
                                        "Failed to encode the snapshot surface as PNG: {}",
                                        e
                                    )),
                                }
                            }
                            Err(e) => Err(format!("WebKitGTK snapshot failed: {}", e)),
                        };

                        // Receiver gone means the caller already timed out.
                        let _ = tx.send(payload);
                    },
                );
            })
            .map_err(|e| {
                Error::WindowOperationFailed(format!(
                    "Could not reach the WebKitGTK webview for window '{}': {}",
                    window_label, e
                ))
            })?;

        let png = match rx.recv_timeout(SNAPSHOT_TIMEOUT) {
            Ok(Ok(png)) => png,
            Ok(Err(message)) => return Err(Error::WindowOperationFailed(message)),
            Err(e) => {
                return Err(Error::WindowOperationFailed(format!(
                    "Gave up after {}s waiting for the WebKitGTK snapshot of window '{}': {}",
                    SNAPSHOT_TIMEOUT.as_secs(),
                    window_label,
                    e
                )))
            }
        };

        // Hand the PNG to the shared pipeline, which re-encodes to the JPEG data
        // URL every platform (and the node bridge) expects.
        let dynamic_image = image::load_from_memory(&png).map_err(|e| {
            Error::WindowOperationFailed(format!(
                "Could not decode the WebKitGTK snapshot ({} bytes of PNG): {}",
                png.len(),
                e
            ))
        })?;

        info!(
            "[TAURI-MCP] Captured webview snapshot: {}x{}",
            dynamic_image.width(),
            dynamic_image.height()
        );

        if dynamic_image.width() == 0 || dynamic_image.height() == 0 {
            return Err(Error::WindowOperationFailed(
                "WebKitGTK snapshot has zero dimensions - is the window mapped?".to_string(),
            ));
        }

        process_image(dynamic_image, &params_clone).map(create_success_response)
    })
    .await
}

// Other Unix flavours (BSD, ...) have no in-process capture path here.
#[cfg(not(target_os = "linux"))]
pub async fn take_screenshot<R: Runtime>(
    _params: ScreenshotParams,
    _window_context: ScreenshotContext<R>,
) -> Result<ScreenshotResponse> {
    handle_screenshot_task(move || {
        Err(Error::WindowOperationFailed(
            "Screenshot capture is not implemented for this Unix platform".to_string(),
        ))
    })
    .await
}

// Add any other Unix-specific functionality here
