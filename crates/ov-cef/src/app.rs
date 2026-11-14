//! The CEF application object.
//!
//! The application object is the place where the command line of every CEF
//! process is configured.  Trap uses it to
//!
//!   * route all traffic through the local SOCKS5 proxy,
//!   * forbid DNS resolutions outside the tunnel,
//!   * switch off WebRTC (it would open a direct path to the clearnet),
//!   * keep the caches in memory only, and
//!   * disable the speculative connections that would leak the visited
//!     hidden service to a third party.

use std::sync::{Arc, Mutex};

use cef::rc::Rc;
use cef::{
    wrap_app, wrap_load_handler, wrap_render_process_handler, App, Browser, CefString, CommandLine,
    Domnode, Frame, ImplApp, ImplCommandLine, ImplFrame, ImplLoadHandler, ImplRenderProcessHandler,
    LoadHandler, ProcessId, ProcessMessage, RenderProcessHandler, WrapApp, WrapLoadHandler,
    WrapRenderProcessHandler,
};

use crate::{
    config::OvConfig,
    dom_pgp::{self, scan_visitor},
    handlers::focus_node,
    ipc::{self, DomFieldMetadata},
};

wrap_app! {
    // The CEF application of Trap.
    pub struct OvApp {
        // The configuration of the engine.
        config: Arc<OvConfig>,
        // The render process handler; it scans the DOM of every document.
        render_process_handler: RenderProcessHandler,
    }

    impl App {
        // Configure the command line of the CEF process.
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };

            // Everything goes through the local SOCKS5 proxy, which tunnels
            // the traffic through Tor.
            command_line.append_switch_with_value(
                Some(&CefString::from("proxy-server")),
                Some(&CefString::from(self.config.proxy_server().as_str())),
            );

            // Resolve every name through the proxy: `MAP * ~NOTFOUND` stops
            // Chromium from asking the system resolver, `EXCLUDE 127.0.0.1`
            // keeps the SOCKS5 listener reachable.
            command_line.append_switch_with_value(
                Some(&CefString::from("host-resolver-rules")),
                Some(&CefString::from(self.config.host_resolver_rules())),
            );

            // WebRTC could open a direct connection to the peer and would
            // leak the real address of the device.
            command_line.append_switch(Some(&CefString::from("disable-webrtc")));

            // No shader cache on disk.
            command_line.append_switch(Some(&CefString::from("disable-gpu-shader-disk-cache")));

            // Ephemeral, in memory only caches.
            command_line.append_switch(Some(&CefString::from("incognito")));

            // No speculative preconnects: they would tell a third party which
            // hidden service is about to be visited.
            command_line.append_switch_with_value(
                Some(&CefString::from("disable-features")),
                Some(&CefString::from("NetworkPrediction")),
            );

            // HTTP/1.1 keeps the connection to the hidden service alive, which
            // matters a lot on a high latency Tor circuit.
            command_line.append_switch(Some(&CefString::from("disable-http2")));

            // No background traffic (update checks, prefetch, ...).
            command_line.append_switch(Some(&CefString::from("disable-background-networking")));
            command_line.append_switch(Some(&CefString::from("no-first-run")));
            command_line.append_switch(Some(&CefString::from("disable-sync")));

            // Android cannot fork a helper process: the CEF subprocesses are
            // started as Android services (see `CefService.kt`), and CEF's
            // Android port looks the service up by this switch.
            command_line.append_switch_with_value(
                Some(&CefString::from("service-class")),
                Some(&CefString::from("dev.onionvault.CefService")),
            );
        }

        // The render process handler.
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(self.render_process_handler.clone())
        }
    }
}

wrap_load_handler! {
    // The load handler of the render process.
    //
    // When the document is ready the DOM is scanned for PGP relevant nodes.
    pub struct OvRenderLoadHandler {
        // The fields that were found by the last scan.
        fields: Arc<Mutex<Vec<DomFieldMetadata>>>,
        // The id of the focused form control.
        focused: Arc<Mutex<Option<String>>>,
    }

    impl LoadHandler {
        /// The document was loaded: scan the DOM.
        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let Some(frame) = frame else {
                return;
            };
            let mut visitor = scan_visitor(
                frame.clone(),
                self.fields.clone(),
                self.focused.clone(),
            );
            frame.visit_dom(Some(&mut visitor));
        }
    }
}

wrap_render_process_handler! {
    // The render process handler.
    //
    // It runs inside the sandboxed render process and is the only place
    // where the DOM can be inspected: the DOM API of CEF is only available
    // there.
    pub struct OvRenderProcessHandler {
        // The fields that were found by the last scan.
        fields: Arc<Mutex<Vec<DomFieldMetadata>>>,
        // The id of the focused form control.
        focused: Arc<Mutex<Option<String>>>,
    }

    impl RenderProcessHandler {
        /// The load handler of the render process: it starts the DOM scan.
        fn load_handler(&self) -> Option<cef::LoadHandler> {
            Some(OvRenderLoadHandler::new(self.fields.clone(), self.focused.clone()))
        }

        /// The user moved the focus to another node.
        fn on_focused_node_changed(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            node: Option<&mut Domnode>,
        ) {
            let Some(frame) = frame else {
                return;
            };
            focus_node(frame, node, self.focused.clone());
        }

        /// The browser process asks the render process to change the DOM.
        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message else {
                return 0;
            };
            let name = ipc::message_name(message);

            match name.as_str() {
                ipc::MSG_MUTATE_DOM_NODE => {
                    if let (Some(frame), Some(request)) =
                        (frame, ipc::read_json::<ipc::MutationRequest>(message))
                    {
                        let mut visitor = dom_pgp::mutation_visitor(
                            frame.clone(),
                            request,
                            self.focused.clone(),
                        );
                        frame.visit_dom(Some(&mut visitor));
                    }
                }
                ipc::MSG_REQ_FIELD_VALUE => {
                    if let (Some(frame), Some(request)) =
                        (frame, ipc::read_json::<ipc::FieldValueRequest>(message))
                    {
                        let mut visitor = dom_pgp::read_value_visitor(
                            frame.clone(),
                            ipc::ValueRequest {
                                node_id: request.node_id,
                            },
                            self.focused.clone(),
                        );
                        frame.visit_dom(Some(&mut visitor));
                    }
                }
                ipc::MSG_READ_VALUE => {
                    // The browser process wants to verify a change that it
                    // made through the input path of the browser.
                    if let (Some(frame), Some(request)) =
                        (frame, ipc::read_json::<ipc::ValueRequest>(message))
                    {
                        let mut visitor = dom_pgp::read_value_visitor(
                            frame.clone(),
                            request,
                            self.focused.clone(),
                        );
                        frame.visit_dom(Some(&mut visitor));
                    }
                }
                ipc::MSG_DECRYPT_RESULT => {
                    if let (Some(frame), Some(response)) =
                        (frame, ipc::read_json::<ipc::DecryptResponse>(message))
                    {
                        // The user asked for an inline replacement: write the
                        // plaintext into the form control.
                        let request = ipc::MutationRequest {
                            node_id: response.node_id.clone(),
                            replacement: response.plaintext.clone(),
                        };
                        let mut visitor = dom_pgp::mutation_visitor(
                            frame.clone(),
                            request,
                            self.focused.clone(),
                        );
                        frame.visit_dom(Some(&mut visitor));
                    }
                }
                other => {
                    log::debug!("ignoring the render process message {other}");
                }
            }
            0
        }
    }
}
