//! The CEF integration of OnionVault.
//!
//! The crate implements
//!
//!   * the CEF application object, which locks down the command line of
//!     every CEF process (proxy, DNS rules, no WebRTC, in memory caches),
//!   * the browser process client, which owns the render, load and request
//!     handlers,
//!   * the render process handler, which scans the DOM for PGP relevant
//!     nodes without ever executing JavaScript, and
//!   * the off-screen render handler, which hands the rendered pixels to the
//!     platform.

pub mod app;
pub mod config;
pub mod dom_model;
pub mod dom_pgp;
pub mod frame_sink;
pub mod handlers;
pub mod ipc;
pub mod keyboard;
pub mod snapshot;

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use cef::{
    App, BrowserSettings, CefString, Client, CommandLine, ImplCommandLine, LogSeverity, Rect,
    RuntimeStyle, Settings, State, WindowInfo,
};

pub use app::{OvApp, OvRenderProcessHandler};
pub use config::{is_allowed_url, is_svg_url, OvConfig};
pub use frame_sink::{FrameSink, PaintBuffer, PixelFormat};
pub use handlers::{
    pending_actions, BrowserBridge, BrowserSlot, NullBridge, OvClient, OvRenderHandler,
    PendingActions,
};
pub use ipc::{
    DecryptRequest, DecryptResponse, DomFieldMetadata, DomScanReport, FieldAction, FieldKind,
    FieldValue, MutationRequest, MutationResult, NavigationInfo, NodeRect,
};
pub use snapshot::{FrameSnapshot, PageSnapshot, SnapshotCollector, MAX_DEPTH};

/// Browser settings of OnionVault.
///
/// JavaScript is switched off in *every* browser instance; the remaining
/// switches remove the features that a hidden service could abuse to
/// fingerprint the device or to open a path around the Tor tunnel.
pub fn browser_settings() -> BrowserSettings {
    BrowserSettings {
        // --- the zero JavaScript guarantee -------------------------------
        javascript: State::DISABLED,
        javascript_close_windows: State::DISABLED,
        javascript_access_clipboard: State::DISABLED,
        javascript_dom_paste: State::DISABLED,

        // --- features that would leak or that are not needed --------------
        databases_deprecated: State::DISABLED,
        webgl: State::DISABLED,
        local_storage: State::DISABLED,
        // Remote fonts would tell the hidden service which fonts the
        // device has.
        remote_fonts: State::DISABLED,

        // Images are needed, the pages of hidden services are mostly images.
        image_loading: State::ENABLED,

        ..Default::default()
    }
}

/// The CEF settings of OnionVault.
///
/// Windowless rendering is mandatory on Android: CEF expects a desktop
/// windowing system (Aura) that Android does not provide.
pub fn cef_settings(
    resources_dir: Option<&Path>,
    locales_dir: Option<&Path>,
    cache_dir: Option<&Path>,
    external_message_pump: bool,
) -> Settings {
    let mut settings = Settings {
        // Off-screen rendering: CEF paints into a memory buffer.
        windowless_rendering_enabled: 1,
        // OnionVault runs its own Tokio runtime; CEF must not install a
        // message pump of its own.
        multi_threaded_message_loop: 0,
        // On Android the message loop of the platform is driven by the
        // native UI (see `MainActivity.kt`), CEF must not install one of its
        // own.
        external_message_pump: if external_message_pump { 1 } else { 0 },
        log_severity: LogSeverity::WARNING,
        ..Default::default()
    };

    if let Some(dir) = resources_dir {
        settings.resources_dir_path = CefString::from(path_to_str(dir));
    }
    if let Some(dir) = locales_dir {
        settings.locales_dir_path = CefString::from(path_to_str(dir));
    }
    if let Some(dir) = cache_dir {
        settings.cache_path = CefString::from(path_to_str(dir));
    }

    settings
}

/// The window information of a windowless browser.
pub fn window_info(width: i32, height: i32) -> WindowInfo {
    WindowInfo {
        windowless_rendering_enabled: 1,
        bounds: Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        // "Alloy" does not draw the browser chrome, which the native UI
        // draws itself.
        runtime_style: RuntimeStyle::ALLOY,
        ..Default::default()
    }
}

/// Build the CEF application and the browser client.
pub fn build_engine(
    config: Arc<OvConfig>,
    bridge: Arc<dyn BrowserBridge>,
    sink: Arc<dyn FrameSink>,
    slot: Arc<BrowserSlot>,
    pending: PendingActions,
    collector: Arc<SnapshotCollector>,
) -> (App, Client) {
    // The changes that the browser process made through the input path of
    // the browser and that the render process still has to confirm.
    let verifications = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let fields = Arc::new(Mutex::new(Vec::new()));

    // The `new` constructor of the wrapper macros already returns the CEF
    // object, so the handlers must not be wrapped a second time.
    let render_handler = handlers::OvRenderHandler::new(sink, collector.clone());
    let browser_load_handler =
        handlers::OvBrowserLoadHandler::new(bridge.clone(), config.clone(), collector.clone());
    let resource_request_handler = handlers::OvResourceRequestHandler::new(config.clone());
    let request_handler = handlers::OvRequestHandler::new(config.clone(), resource_request_handler);
    let life_span_handler = handlers::OvLifeSpanHandler::new(slot);

    let client = OvClient::new(
        bridge,
        render_handler,
        browser_load_handler,
        request_handler,
        life_span_handler,
        pending,
        verifications,
        collector,
    );

    // The id of the focused form control; it is shared between the scan and
    // the mutation of the render process.
    let focused = Arc::new(Mutex::new(None));
    let render_process_handler = OvRenderProcessHandler::new(fields, focused);
    let app = OvApp::new(config, render_process_handler);

    (app, client)
}

/// Append the OnionVault switches to a command line.
///
/// This is used for the CEF subprocesses that OnionVault starts itself (the
/// renderer, the GPU and the utility process are started through Android
/// services, see `CefService.kt`).
pub fn apply_command_line(command_line: &mut CommandLine, config: &OvConfig) {
    command_line.append_switch_with_value(
        Some(&CefString::from("proxy-server")),
        Some(&CefString::from(config.proxy_server().as_str())),
    );
    command_line.append_switch_with_value(
        Some(&CefString::from("host-resolver-rules")),
        Some(&CefString::from(config.host_resolver_rules())),
    );
    command_line.append_switch(Some(&CefString::from("disable-webrtc")));
    command_line.append_switch(Some(&CefString::from("disable-gpu-shader-disk-cache")));
    command_line.append_switch(Some(&CefString::from("incognito")));
    command_line.append_switch_with_value(
        Some(&CefString::from("disable-features")),
        Some(&CefString::from("NetworkPrediction")),
    );
    command_line.append_switch(Some(&CefString::from("disable-http2")));
    command_line.append_switch(Some(&CefString::from("disable-background-networking")));
    command_line.append_switch(Some(&CefString::from("no-first-run")));
}

/// Convert a path into the string representation that CEF expects.
fn path_to_str(path: &Path) -> &str {
    path.to_str().unwrap_or("")
}
