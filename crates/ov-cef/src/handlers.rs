//! The CEF handlers of OnionVault.
//!
//! * [`OvClient`] is the browser process client: it owns the render handler,
//!   the load handler and the request handler and it receives the messages
//!   that the render process sends.
//! * [`OvRenderHandler`] copies the off-screen rendered pixels into the
//!   platform surface.
//! * [`OvRequestHandler`] and [`OvResourceRequestHandler`] implement the leak
//!   prevention: in strict hidden service mode every clearnet request is
//!   cancelled and the identifying headers are stripped.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cef::rc::Rc;
use cef::{
    wrap_client, wrap_life_span_handler, wrap_load_handler, wrap_render_handler,
    wrap_request_handler, wrap_resource_request_handler, Browser, Callback, CefString, Client,
    Domnode, Frame, ImplCallback, ImplClient, ImplFrame, ImplLifeSpanHandler, ImplLoadHandler,
    ImplRenderHandler, ImplRequest, ImplRequestHandler, ImplResourceRequestHandler,
    LifeSpanHandler, LoadHandler, PaintElementType, ProcessId, ProcessMessage, Rect, RenderHandler,
    Request, RequestHandler, ResourceRequestHandler, ReturnValue, WrapClient, WrapLifeSpanHandler,
    WrapLoadHandler, WrapRenderHandler, WrapRequestHandler, WrapResourceRequestHandler,
};

use crate::{
    config::{is_allowed_url, is_svg_url, OvConfig},
    dom_pgp::{self, scan_visitor},
    frame_sink::FrameSink,
    ipc::{self, DomScanReport, NavigationInfo},
    keyboard,
    snapshot::{PageSnapshot, SnapshotCollector},
};

/// The bridge between CEF and the rest of the application.
///
/// The browser process uses it to report the detected DOM fields, to request
/// a decryption and to report navigations.  `ov-core` implements it on top of
/// the OpenPGP key manager and the JNI bridge to the native Android UI.
pub trait BrowserBridge: Send + Sync {
    /// The render process found new DOM fields.
    fn on_dom_scan(&self, report: DomScanReport);
    /// The render process asks for the decryption of an armored message.
    ///
    /// The response is sent back to the render process; when the plaintext
    /// must not be written into the page, the implementation shows it in the
    /// native sheet instead and returns `None`.
    fn on_decrypt_request(&self, request: ipc::DecryptRequest) -> Option<ipc::DecryptResponse>;
    /// The render process sent the value of a node.
    ///
    /// The implementation performs the requested action and returns the text
    /// that has to replace the value of the node.
    fn on_field_value(
        &self,
        value: ipc::FieldValue,
        action: ipc::FieldAction,
        recipients: &[String],
    ) -> Option<String>;
    /// The render process reported the outcome of a mutation.
    ///
    /// The implementation tells the user whether the page carries the new
    /// content: a field that still holds the old text would send it.
    fn on_mutation_result(&self, result: ipc::MutationResult);
    /// A page was left and its snapshot is complete.
    ///
    /// The implementation tells the native UI that the back stack changed.
    fn on_snapshot(&self, snapshot: PageSnapshot);
    /// The browser navigated to a new document.
    fn on_navigation(&self, info: NavigationInfo);
}

/// A bridge that does nothing; used when CEF runs without the native UI.
pub struct NullBridge;

impl BrowserBridge for NullBridge {
    fn on_dom_scan(&self, _report: DomScanReport) {}
    fn on_decrypt_request(&self, _request: ipc::DecryptRequest) -> Option<ipc::DecryptResponse> {
        None
    }
    fn on_field_value(
        &self,
        _value: ipc::FieldValue,
        _action: ipc::FieldAction,
        _recipients: &[String],
    ) -> Option<String> {
        None
    }
    fn on_mutation_result(&self, _result: ipc::MutationResult) {}
    fn on_snapshot(&self, _snapshot: PageSnapshot) {}
    fn on_navigation(&self, _info: NavigationInfo) {}
}

/// The registry of the field actions that the native UI requested.
///
/// The native UI asks the render process for the value of a node; the
/// answer arrives asynchronously, so the browser process has to remember
/// what it wanted to do with the value.
pub type PendingActions = Arc<Mutex<HashMap<String, (ipc::FieldAction, Vec<String>)>>>;

/// Create an empty registry of pending field actions.
pub fn pending_actions() -> PendingActions {
    Arc::new(Mutex::new(HashMap::new()))
}

/// The slot that holds the browser once CEF created it.
///
/// The browser is created asynchronously (the CEF UI thread owns it), so the
/// application learns about it through the life span handler.
#[derive(Default)]
pub struct BrowserSlot {
    browser: std::sync::Mutex<Option<Browser>>,
}

impl BrowserSlot {
    /// Create an empty slot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store the browser.
    pub fn set(&self, browser: Browser) {
        *self.browser.lock().unwrap() = Some(browser);
    }

    /// The browser, if it was created already.
    pub fn get(&self) -> Option<Browser> {
        self.browser.lock().unwrap().clone()
    }
}

wrap_life_span_handler! {
    // The life span handler of OnionVault.
    //
    // It stores the browser in the [`BrowserSlot`] and blocks popups: a
    // hidden service must not be able to open a second window.
    pub struct OvLifeSpanHandler {
        // The slot the browser is stored in.
        slot: Arc<BrowserSlot>,
    }

    impl LifeSpanHandler {
        // The browser was created.
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                self.slot.set(browser.clone());
            }
        }

        // Popups are not allowed.
        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            target_frame_name: Option<&CefString>,
            target_disposition: cef::WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
            popup_features: Option<&cef::PopupFeatures>,
            window_info: Option<&mut cef::WindowInfo>,
            client: Option<&mut Option<Client>>,
            settings: Option<&mut cef::BrowserSettings>,
            extra_info: Option<&mut Option<cef::DictionaryValue>>,
            no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            log::warn!("blocked a popup");
            1
        }
    }
}

wrap_client! {
    // The browser process client of OnionVault.
    pub struct OvClient {
        // The bridge to the native application.
        bridge: Arc<dyn BrowserBridge>,
        // The render handler that receives the off-screen frames.
        render_handler: RenderHandler,
        // The load handler of the browser process.
        load_handler: LoadHandler,
        // The request handler that implements the leak prevention.
        request_handler: RequestHandler,
        // The life span handler that stores the browser.
        life_span_handler: LifeSpanHandler,
        // The field actions that the native UI requested.
        pending: PendingActions,
        // The changes that the browser process made through the input path
        // and that the render process still has to confirm.
        verifications: Arc<Mutex<HashMap<String, String>>>,
        // The collector of the page snapshots.
        collector: Arc<SnapshotCollector>,
    }

    impl Client {
        /// The off-screen render handler.
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }

        /// The load handler of the browser process.
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load_handler.clone())
        }

        /// The request handler of the browser process.
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.request_handler.clone())
        }

        /// The life span handler of the browser process.
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }

        /// Handles the messages that the render process sends.
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
                ipc::MSG_DOM_FIELDS => {
                    if let Some(report) = ipc::read_json::<DomScanReport>(message) {
                        self.collector.update_scan(
                            &report.title,
                            report.markup.clone(),
                            report.fields.clone(),
                        );
                        self.bridge.on_dom_scan(report);
                    }
                }
                ipc::MSG_FIELD_VALUE => {
                    if let Some(value) = ipc::read_json::<ipc::FieldValue>(message) {
                        // Is this the confirmation of a change that the
                        // browser process made through the input path?
                        let intended = {
                            let mut verifications = self.verifications.lock().unwrap();
                            verifications.remove(&value.node_id)
                        };
                        if let Some(intended) = intended {
                            confirm_mutation(
                                &self.bridge,
                                value.node_id.clone(),
                                &value.value,
                                &intended,
                            );
                            return 0;
                        }

                        let action = {
                            let mut pending = self.pending.lock().unwrap();
                            pending.remove(&value.node_id)
                        };
                        if let Some((action, recipients)) = action {
                            if let Some(replacement) =
                                self.bridge.on_field_value(value.clone(), action, &recipients)
                            {
                                if let Some(frame) = frame {
                                    let request = ipc::MutationRequest {
                                        node_id: value.node_id.clone(),
                                        replacement,
                                    };
                                    ipc::send_json_to_renderer(
                                        frame,
                                        ipc::MSG_MUTATE_DOM_NODE,
                                        &request,
                                    );
                                }
                            }
                        } else {
                            log::warn!(
                                "no pending action for the node {}",
                                value.node_id
                            );
                        }
                    }
                }
                ipc::MSG_REQ_DECRYPT => {
                    if let Some(request) = ipc::read_json::<ipc::DecryptRequest>(message) {
                        if let Some(response) = self.bridge.on_decrypt_request(request) {
                            if let Some(frame) = frame {
                                ipc::send_json_to_renderer(
                                    frame,
                                    ipc::MSG_DECRYPT_RESULT,
                                    &response,
                                );
                            }
                        }
                    }
                }
                ipc::MSG_MUTATION_RESULT => {
                    if let Some(outcome) = ipc::read_json::<ipc::MutationResult>(message) {
                        finish_mutation(
                            &self.bridge,
                            &self.verifications,
                            browser,
                            frame,
                            outcome,
                        );
                    }
                }
                other => {
                    log::debug!("ignoring the process message {other}");
                }
            }
            0
        }
    }
}

/// Finish a mutation that the DOM API could not complete.
///
/// A control that the user has typed into carries a dirty value that the DOM
/// API cannot replace; the input path of the browser can, but only while the
/// control is the focused one.  The change is confirmed by the render
/// process, which reads the live value of the node again; the outcome is
/// reported to the user in every case, because a field that still holds the
/// old text would send it.
fn finish_mutation(
    bridge: &Arc<dyn BrowserBridge>,
    verifications: &Arc<Mutex<HashMap<String, String>>>,
    browser: Option<&mut Browser>,
    frame: Option<&mut Frame>,
    outcome: ipc::MutationResult,
) {
    let mut outcome = outcome;
    if outcome.needs_keyboard && outcome.focused {
        if let (Some(browser), Some(frame)) = (browser, frame) {
            if keyboard::replace_focused_text(browser, frame, &outcome.text) {
                // The render process confirms the change by reading the live
                // value of the node again.
                verifications
                    .lock()
                    .unwrap()
                    .insert(outcome.node_id.clone(), outcome.text.clone());
                let request = ipc::ValueRequest {
                    node_id: outcome.node_id.clone(),
                };
                ipc::send_json_to_renderer(frame, ipc::MSG_READ_VALUE, &request);
                return;
            }
            outcome.reason = "the focused field could not be changed".to_string();
        }
    }
    bridge.on_mutation_result(outcome);
}

/// Report the confirmation of a change that the browser process made.
fn confirm_mutation(bridge: &Arc<dyn BrowserBridge>, node_id: String, live: &str, intended: &str) {
    // CEF trims the leading whitespace of the value of a form control; the
    // comparison uses the same normalisation.
    let applied = live.trim_start() == intended.trim_start();
    let reason = if applied {
        String::new()
    } else {
        "the field does not carry the new text".to_string()
    };
    log::info!(
        "the change of the node {node_id} was {}",
        if applied { "confirmed" } else { "refused" }
    );
    bridge.on_mutation_result(ipc::MutationResult {
        node_id,
        applied,
        needs_keyboard: false,
        focused: true,
        reason,
        text: String::new(),
    });
}

wrap_render_handler! {
    // The off-screen render handler.
    //
    // CEF renders the page into a memory buffer; the buffer is handed to the
    // platform, which copies it into the Android native window.
    pub struct OvRenderHandler {
        // The sink that receives the frames.
        sink: Arc<dyn FrameSink>,
        // The collector of the page snapshots.
        collector: Arc<SnapshotCollector>,
    }

    impl RenderHandler {
        /// CEF asks for the size of the view it has to render into.
        fn view_rect(&self, browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let (width, height) = self.sink.viewport();
            if let Some(rect) = rect {
                rect.x = 0;
                rect.y = 0;
                rect.width = width;
                rect.height = height;
            }
        }

        /// A frame was rendered.
        fn on_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let stride = width as usize * 4;
            let len = stride * height as usize;
            // SAFETY: CEF guarantees that the buffer holds `width * height`
            // pixels of four bytes each for the duration of the callback.
            let pixels = unsafe { std::slice::from_raw_parts(buffer, len) };
            let dirty: &[Rect] = dirty_rects.unwrap_or(&[]);
            self.sink.present(crate::frame_sink::PaintBuffer {
                pixels,
                width,
                height,
                stride,
                dirty_rects: dirty,
            });

            // The frame is kept for the snapshot of the page.
            self.collector.update_frame(
                pixels,
                width,
                height,
                stride,
                self.sink.pixel_format(),
            );
        }

        /// The document was scrolled.
        fn on_scroll_offset_changed(
            &self,
            browser: Option<&mut Browser>,
            x: f64,
            y: f64,
        ) {
            self.collector.update_scroll(x, y);
        }
    }
}

wrap_load_handler! {
    // The load handler of the browser process.
    pub struct OvBrowserLoadHandler {
        // The bridge to the native application.
        bridge: Arc<dyn BrowserBridge>,
        // The configuration.
        config: Arc<OvConfig>,
        // The collector of the page snapshots.
        collector: Arc<SnapshotCollector>,
    }

    impl LoadHandler {
        /// The document was loaded.
        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let Some(frame) = frame else {
                return;
            };
            let url = dom_pgp::userfree_to_string(&frame.url());
            let blocked = self.config.strict_hidden_service && !is_allowed_url(&url);

            // The page that was shown until now is complete: its snapshot is
            // handed to the native UI, which shows it in the preview of the
            // predictive back gesture.
            if let Some(previous) = self.collector.begin_page(&url) {
                self.bridge.on_snapshot(previous);
            }

            self.bridge.on_navigation(NavigationInfo {
                url,
                title: String::new(),
                blocked,
            });
        }
    }
}

wrap_request_handler! {
    // The request handler of the browser process.
    pub struct OvRequestHandler {
        // The configuration.
        config: Arc<OvConfig>,
        // The resource request handler that inspects the subresources.
        resource_request_handler: ResourceRequestHandler,
    }

    impl RequestHandler {
        /// Every request is passed to the resource request handler.
        fn resource_request_handler(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            is_navigation: ::std::os::raw::c_int,
            is_download: ::std::os::raw::c_int,
            request_initiator: Option<&CefString>,
            disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            Some(self.resource_request_handler.clone())
        }

        /// Block clearnet navigations in strict hidden service mode.
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            if !self.config.strict_hidden_service {
                return 0;
            }
            let Some(request) = request else {
                return 0;
            };
            let url = dom_pgp::userfree_to_string(&request.url());
            if is_allowed_url(&url) {
                return 0;
            }
            log::warn!("blocked the clearnet navigation to {url}");
            1
        }
    }
}

wrap_resource_request_handler! {
    // The resource request handler.
    //
    // It strips the identifying headers from every request and cancels the
    // clearnet subresources in strict hidden service mode.
    pub struct OvResourceRequestHandler {
        // The configuration.
        config: Arc<OvConfig>,
    }

    impl ResourceRequestHandler {
        /// Inspect a request before it is sent.
        fn on_before_resource_load(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let Some(request) = request else {
                return ReturnValue::CONTINUE;
            };

            let url = dom_pgp::userfree_to_string(&request.url());

            // Strip the headers that identify the user or the origin.
            for header in ["User-Agent", "Accept-Language", "Referer"] {
                request.set_header_by_name(
                    Some(&CefString::from(header)),
                    Some(&CefString::from("")),
                    1,
                );
            }

            if self.config.strict_hidden_service && !is_allowed_url(&url) {
                log::warn!("blocked the clearnet request to {url}");
                if let Some(callback) = callback {
                    callback.cancel();
                }
                return ReturnValue::CANCEL;
            }

            if self.config.block_svg && is_svg_url(&url) {
                log::debug!("blocked the SVG resource {url}");
                if let Some(callback) = callback {
                    callback.cancel();
                }
                return ReturnValue::CANCEL;
            }

            ReturnValue::CONTINUE
        }
    }
}

/// Mark the focused DOM node and rescan the document.
///
/// The scan remembers the focused form control: a control that the user has
/// typed into keeps a dirty value that only the input path of the browser
/// can replace.
pub fn focus_node(frame: &Frame, node: Option<&mut Domnode>, focused: Arc<Mutex<Option<String>>>) {
    if let Some(node) = node {
        dom_pgp::mark_focused(node);
    } else if let Ok(mut focused) = focused.lock() {
        // The focus was lost: no control can be changed through the input
        // path any more.
        *focused = None;
    }
    let mut visitor = scan_visitor(frame.clone(), Arc::new(Mutex::new(Vec::new())), focused);
    frame.visit_dom(Some(&mut visitor));
}
