//! The native entry point of Trap.
//!
//! The crate glues the four engines together:
//!
//!   * `ov-tor` runs the embedded Tor client and its SOCKS5 listener,
//!   * `ov-cef` runs the off-screen rendered CEF browser and the native DOM
//!     PGP engine,
//!   * `ov-pgp` runs the hardware backed OpenPGP key manager, and
//!   * this crate owns the Android specifics: the JNI entry points, the
//!     native window and the translation of the Android input events.

pub mod bridge;
pub mod events;
pub mod native;
pub mod state;
pub mod surface;

use std::{
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

#[cfg(target_os = "android")]
use std::path::PathBuf;

use anyhow::{Context, Result};
use cef::{CefString, ImplBrowser, ImplBrowserHost, ImplFrame};
use ov_cef::{BrowserSlot, OvConfig};
use ov_pgp::{master_key_provider, Keyring, KeyringStore};

pub use bridge::OvBridge;
pub use state::EngineState;
pub use surface::SurfaceRenderer;

/// The global engine instance.
static ENGINE: OnceLock<Mutex<Option<Arc<EngineState>>>> = OnceLock::new();

/// The Tokio runtime that carries the Tor client.
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// The alias of the master wrapping key in the Android Keystore.
const MASTER_KEY_ALIAS: &str = "onionvault.master.wrap.v1";

/// The engine of the browser.
pub struct Engine {
    /// The shared state.
    state: Arc<EngineState>,
}

impl Engine {
    /// Start the Tor client, the key manager and the CEF engine.
    ///
    /// The function has to run on the thread that becomes the CEF UI thread
    /// (the Android main thread).
    pub fn start(
        files_dir: &Path,
        hidden_service_only: bool,
        require_user_authentication: bool,
    ) -> Result<Arc<Self>> {
        let runtime = RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("ov-runtime")
                .build()
                .expect("cannot create the Tokio runtime")
        });

        // --- the Tor transport ------------------------------------------------
        let tor_config = ov_tor::TorConfig {
            state_dir: files_dir.join("arti_state"),
            cache_dir: files_dir.join("arti_cache"),
            allow_clearnet: !hidden_service_only,
            ..Default::default()
        };
        let tor = runtime
            .block_on(ov_tor::TorService::start(tor_config))
            .context("cannot start the embedded Tor client")?;
        log::info!("the Tor proxy listens on {}", tor.socks_endpoint());

        // Report the bootstrap progress to the native UI.
        {
            let tor = tor.clone();
            runtime.spawn(async move {
                let mut events = tor.subscribe();
                loop {
                    match events.recv().await {
                        Ok(progress) => {
                            native::call_native(
                                "onTorProgress",
                                &[
                                    native::NativeArg::Int(progress.percent as i32),
                                    native::NativeArg::String(progress.stage.clone()),
                                ],
                            );
                            if progress.ready {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
        }

        // --- the OpenPGP key manager ------------------------------------------
        let store = KeyringStore::open(&files_dir.join("pgp").join("keyring.db"))
            .context("cannot open the keyring database")?;
        #[cfg(target_os = "android")]
        let master = {
            let vm = crate::native::java_vm()
                .context("the Java VM is not available: cannot open the Keystore")?;
            master_key_provider(vm, MASTER_KEY_ALIAS, require_user_authentication)
                .context("cannot obtain the master wrapping key")?
        };
        #[cfg(not(target_os = "android"))]
        let master = master_key_provider(MASTER_KEY_ALIAS, require_user_authentication)
            .context("cannot obtain the master wrapping key")?;
        log::info!("the master key is provided by {}", master.describe());
        let keyring = Arc::new(Keyring::open(store, master));

        // --- the CEF engine ----------------------------------------------------
        let config = Arc::new(OvConfig {
            proxy_endpoint: tor.socks_endpoint(),
            hidden_service_only,
            block_svg: false,
        });

        let surface = Arc::new(SurfaceRenderer::new(ov_cef::PixelFormat::Rgba8888));
        let slot = Arc::new(BrowserSlot::new());
        let pending = ov_cef::pending_actions();

        // The collector of the page snapshots; the native UI pulls the
        // snapshots from it for the preview of the predictive back gesture.
        let snapshots = Arc::new(ov_cef::SnapshotCollector::new());

        let state = Arc::new(EngineState {
            slot: slot.clone(),
            keyring,
            tor,
            surface: surface.clone(),
            config: config.clone(),
            pending: pending.clone(),
            snapshots: snapshots.clone(),
        });

        let bridge = Arc::new(OvBridge::new(state.clone()));
        let (mut app, mut client) = ov_cef::build_engine(
            config,
            bridge,
            surface.clone(),
            slot.clone(),
            pending.clone(),
            snapshots.clone(),
        );

        // The CEF resources were unpacked from the APK assets by the Kotlin
        // side into `filesDir/cef_resources`.
        let resources_dir = files_dir.join("cef_resources");
        let locales_dir = resources_dir.join("locales");
        let settings = ov_cef::cef_settings(
            Some(&resources_dir),
            Some(&locales_dir),
            Some(&files_dir.join("cef_cache")),
            true,
        );

        let args = build_main_args();
        let result = cef::initialize(
            Some(&args),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if result != 0 {
            anyhow::bail!("CefInitialize failed with {result}");
        }
        log::info!("CEF was initialized");

        // Create the windowless browser.
        let (width, height) = surface.size();
        let info = ov_cef::window_info(width.max(1), height.max(1));
        let url = CefString::from("about:blank");
        let browser = cef::browser_host_create_browser(
            Some(&info),
            Some(&mut client),
            Some(&url),
            Some(&ov_cef::browser_settings()),
            None,
            None,
        );
        if browser == 0 {
            anyhow::bail!("CefBrowserHost::CreateBrowser failed");
        }

        let engine = Arc::new(Self { state });
        *ENGINE.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(engine.state.clone());

        Ok(engine)
    }

    /// The shared state of the engine.
    pub fn state(&self) -> Arc<EngineState> {
        self.state.clone()
    }

    /// Stop the engine.
    pub fn shutdown(&self) {
        if let Some(browser) = self.state.browser() {
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
        self.state.tor.shutdown();
    }
}

/// Build the `argv` that CEF expects.
///
/// Android does not give the native library a real command line, so a
/// synthetic one is used.
fn build_main_args() -> cef::MainArgs {
    use std::{ffi::CString, os::raw::c_char};

    let mut argv = [CString::new("onionvault").expect("valid program name")];
    let mut pointers: Vec<*mut c_char> = argv
        .iter_mut()
        .map(|value| value.as_ptr() as *mut c_char)
        .collect();

    cef::MainArgs {
        argc: pointers.len() as std::os::raw::c_int,
        argv: pointers.as_mut_ptr(),
    }
}

/// The engine instance, if it was started.
pub fn engine() -> Option<Arc<EngineState>> {
    ENGINE.get().and_then(|slot| slot.lock().unwrap().clone())
}

/// Load a URL in the browser.
pub fn load_url(url: &str) -> Result<()> {
    let state = engine().ok_or_else(|| anyhow::anyhow!("the engine is not running"))?;
    let browser = state
        .browser()
        .ok_or_else(|| anyhow::anyhow!("the browser was not created yet"))?;
    let frame = browser
        .main_frame()
        .ok_or_else(|| anyhow::anyhow!("the browser has no main frame"))?;
    frame.load_url(Some(&cef::CefString::from(url)));
    Ok(())
}

/// The JNI entry points of Trap.
/// The JNI entry points of the native library.
///
/// The names of the functions are fixed by the JNI ABI.
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod jni_api {
    use super::*;
    use jni::objects::{JObject, JString};
    use jni::sys::{jboolean, jint, jstring};
    use jni::JNIEnv;

    /// Cast the JNI environment pointer into the type of the NDK.
    ///
    /// The NDK declares the type privately, so it cannot be named here.
    fn as_ndk_env<T>(pointer: &mut *mut jni::sys::JNIEnv) -> *mut T {
        pointer as *mut *mut jni::sys::JNIEnv as *mut T
    }

    /// Read a Java string into a Rust `String`.
    fn read_string(env: &mut JNIEnv, value: jstring) -> Result<String> {
        if value.is_null() {
            anyhow::bail!("the string argument is null");
        }
        // The argument is a local reference of the calling frame.
        let value = unsafe { JString::from_raw(value) };
        let text = env
            .get_string(&value)
            .context("cannot read the string argument")?
            .into();
        Ok(text)
    }

    /// Start the engine.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInit(
        mut env: JNIEnv,
        _this: JObject,
        files_dir: jstring,
        _cache_dir: jstring,
        hidden_service_only: jboolean,
        require_auth: jboolean,
    ) -> jboolean {
        let files_dir = match read_string(&mut env, files_dir) {
            Ok(value) => PathBuf::from(value),
            Err(err) => {
                log::error!("{err:#}");
                return 0;
            }
        };

        match env.get_java_vm() {
            Ok(vm) => native::set_java_vm(vm),
            Err(err) => log::error!("cannot obtain the Java VM: {err}"),
        }

        match Engine::start(&files_dir, hidden_service_only != 0, require_auth != 0) {
            Ok(_) => 1,
            Err(err) => {
                log::error!("cannot start the engine: {err:#}");
                native::call_native(
                    "showError",
                    &[native::NativeArg::String(format!("{err:#}"))],
                );
                0
            }
        }
    }

    /// Stop the engine.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeShutdown(
        _env: JNIEnv,
        _this: JObject,
    ) {
        if let Some(state) = engine() {
            if let Some(browser) = state.browser() {
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            }
            state.tor.shutdown();
        }
        cef::shutdown();
    }

    /// Attach the Android surface to the renderer.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeCreateSurface(
        _env: JNIEnv,
        _this: JObject,
        surface: JObject,
        width: jint,
        height: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(env_ptr) = native::jni_env_pointer() else {
            log::error!("cannot obtain the JNI environment of the thread");
            return;
        };
        // The NDK expects the address of the JNI environment pointer.
        let mut env_ptr = env_ptr;
        let window = unsafe {
            ndk::native_window::NativeWindow::from_surface(
                as_ndk_env(&mut env_ptr),
                surface.as_raw(),
            )
        };
        match window {
            Some(window) => {
                state.surface.attach(window, width, height);
                log::info!("the Android surface is attached ({width}x{height})");
            }
            None => log::error!("cannot obtain the native window of the surface"),
        }
    }

    /// Detach the Android surface.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDestroySurface(
        _env: JNIEnv,
        _this: JObject,
    ) {
        if let Some(state) = engine() {
            state.surface.detach();
        }
    }

    /// Resize the browser.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeResize(
        _env: JNIEnv,
        _this: JObject,
        width: jint,
        height: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        state.surface.resize(width, height);
        if let Some(browser) = state.browser() {
            if let Some(host) = browser.host() {
                host.was_resized();
                host.notify_screen_info_changed();
            }
        }
    }

    /// Run one iteration of the CEF message loop.
    ///
    /// CEF runs with an external message pump on Android: the native UI
    /// calls this function from the `Choreographer` of the main thread.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDoMessageLoopWork(
        _env: JNIEnv,
        _this: JObject,
    ) {
        cef::do_message_loop_work();
    }

    /// Load a URL.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeLoadUrl(
        mut env: JNIEnv,
        _this: JObject,
        url: jstring,
    ) {
        let url = match read_string(&mut env, url) {
            Ok(value) => value,
            Err(err) => {
                log::error!("{err:#}");
                return;
            }
        };
        if let Err(err) = load_url(&url) {
            log::error!("cannot load {url}: {err:#}");
        }
    }

    /// The number of snapshots of the back stack.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotCount(
        _env: JNIEnv,
        _this: JObject,
    ) -> jint {
        let Some(state) = engine() else {
            return 0;
        };
        state.snapshots.len() as jint
    }

    /// The metadata of a snapshot of the back stack.
    ///
    /// The payload is a JSON document with the URL, the title, the scroll
    /// offset, the size of the frame and the PGP fields of the page.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotInfo(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jstring {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let stack = state.snapshots.stack();
        let Some(entry) = stack.get(index.max(0) as usize) else {
            return std::ptr::null_mut();
        };

        let (viewport_width, viewport_height) = state.surface.size();
        let payload = serde_json::json!({
            "url": entry.url,
            "title": entry.title,
            "scroll_x": entry.scroll_x,
            "scroll_y": entry.scroll_y,
            "frame_width": entry.frame.as_ref().map(|frame| frame.width).unwrap_or(0),
            "frame_height": entry.frame.as_ref().map(|frame| frame.height).unwrap_or(0),
            "viewport_width": viewport_width,
            "viewport_height": viewport_height,
            "fields": entry.fields,
        });
        let text = payload.to_string();

        match env.new_string(text) {
            Ok(value) => value.into_raw(),
            Err(err) => {
                log::error!("cannot create the snapshot info: {err}");
                std::ptr::null_mut()
            }
        }
    }

    /// The pixels of the frame of a snapshot.
    ///
    /// The pixels are in the `R`, `G`, `B`, `A` order and are reduced to a
    /// quarter of the size of the page.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotFrame(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jni::sys::jbyteArray {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let stack = state.snapshots.stack();
        let Some(entry) = stack.get(index.max(0) as usize) else {
            return std::ptr::null_mut();
        };
        let Some(frame) = entry.frame.as_ref() else {
            return std::ptr::null_mut();
        };

        match env.byte_array_from_slice(&frame.pixels) {
            Ok(array) => array.into_raw(),
            Err(err) => {
                log::error!("cannot create the snapshot frame: {err}");
                std::ptr::null_mut()
            }
        }
    }

    /// The markup of a snapshot of the back stack.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotMarkup(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jni::sys::jbyteArray {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let stack = state.snapshots.stack();
        let Some(entry) = stack.get(index.max(0) as usize) else {
            return std::ptr::null_mut();
        };

        match env.byte_array_from_slice(&entry.markup) {
            Ok(array) => array.into_raw(),
            Err(err) => {
                log::error!("cannot create the snapshot markup: {err}");
                std::ptr::null_mut()
            }
        }
    }

    /// A boolean of CEF.
    ///
    /// The bindings of CEF return a boolean as `bool` on some platforms and
    /// as an integer on others; the code of the engine works with both.
    pub trait CefBool {
        /// The value as a boolean.
        fn cef_truthy(&self) -> bool;
    }

    impl CefBool for bool {
        fn cef_truthy(&self) -> bool {
            *self
        }
    }

    impl CefBool for i32 {
        fn cef_truthy(&self) -> bool {
            *self != 0
        }
    }

    impl CefBool for u32 {
        fn cef_truthy(&self) -> bool {
            *self != 0
        }
    }

    impl CefBool for i64 {
        fn cef_truthy(&self) -> bool {
            *self != 0
        }
    }

    /// Go back in the history of the browser.
    ///
    /// The native UI shows the snapshot of the previous page right away, so
    /// the latency of the hidden service is hidden behind the preview.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGoBack(
        _env: JNIEnv,
        _this: JObject,
    ) -> jboolean {
        let Some(state) = engine() else {
            return 0;
        };
        let Some(browser) = state.browser() else {
            return 0;
        };
        if !browser.can_go_back().cef_truthy() {
            return 0;
        }
        browser.go_back();
        1
    }

    /// Forward a touch event.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeTouch(
        _env: JNIEnv,
        _this: JObject,
        action: jint,
        x: f32,
        y: f32,
        id: jint,
        modifiers: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let Some(action) = events::TouchAction::from_android(action) else {
            return;
        };
        let event = events::touch_event(action, x, y, id, modifiers as u32);
        host.send_touch_event(Some(&event));
    }

    /// Forward a mouse button event.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeMouseButton(
        _env: JNIEnv,
        _this: JObject,
        x: f32,
        y: f32,
        button: jint,
        mouse_up: jint,
        modifiers: jint,
        click_count: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let event = events::mouse_event(x, y, modifiers as u32);
        host.send_mouse_click_event(
            Some(&event),
            events::mouse_button(button),
            mouse_up,
            click_count,
        );
    }

    /// Forward a mouse move event.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeMouseMove(
        _env: JNIEnv,
        _this: JObject,
        x: f32,
        y: f32,
        modifiers: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let event = events::mouse_event(x, y, modifiers as u32);
        host.send_mouse_move_event(Some(&event), 0);
    }

    /// Forward a scroll event.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeMouseWheel(
        _env: JNIEnv,
        _this: JObject,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        modifiers: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let event = events::mouse_event(x, y, modifiers as u32);
        host.send_mouse_wheel_event(Some(&event), delta_x as i32, delta_y as i32);
    }

    /// Forward a key event.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeKey(
        _env: JNIEnv,
        _this: JObject,
        action: jint,
        key_code: jint,
        character: jint,
        modifiers: jint,
        is_system_key: jint,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let action = match action {
            0 => events::KeyAction::Down,
            1 => events::KeyAction::Up,
            _ => events::KeyAction::Char,
        };
        let event = events::key_event(
            action,
            key_code,
            character as u16,
            modifiers as u32,
            is_system_key != 0,
        );
        host.send_key_event(Some(&event));
    }

    /// Commit the text of the soft keyboard.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeImeCommitText(
        mut env: JNIEnv,
        _this: JObject,
        text: jstring,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(host) = browser.host() else {
            return;
        };
        let Ok(text) = read_string(&mut env, text) else {
            return;
        };
        host.ime_commit_text(Some(&cef::CefString::from(text.as_str())), None, 0);
    }

    /// Write the plaintext of a decrypted message into the page.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInsertPlaintext(
        mut env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        text: jstring,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(frame) = browser.main_frame() else {
            return;
        };
        let (Ok(node_id), Ok(text)) = (read_string(&mut env, node_id), read_string(&mut env, text))
        else {
            return;
        };
        let request = ov_cef::MutationRequest {
            node_id,
            replacement: text,
        };
        ov_cef::ipc::send_json_to_renderer(&frame, ov_cef::ipc::MSG_MUTATE_DOM_NODE, &request);
    }

    /// Encrypt the content of a field.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeEncryptField(
        mut env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        recipients: jni::objects::JObjectArray<'static>,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(frame) = browser.main_frame() else {
            return;
        };
        let Ok(node_id) = read_string(&mut env, node_id) else {
            return;
        };

        let mut fingerprints = Vec::new();
        let count = env.get_array_length(&recipients).unwrap_or(0);
        for index in 0..count {
            let Ok(recipient) = env.get_object_array_element(&recipients, index) else {
                continue;
            };
            if let Ok(value) = env.get_string(&JString::from(recipient)) {
                fingerprints.push(value.into());
            }
        }

        // Remember what the native UI wants to do with the value.
        register_pending(&node_id, ov_cef::ipc::FieldAction::Encrypt, fingerprints);

        let request = ov_cef::ipc::FieldValueRequest {
            node_id,
            action: ov_cef::ipc::FieldAction::Encrypt,
            recipients: Vec::new(),
        };
        ov_cef::ipc::send_json_to_renderer(&frame, ov_cef::ipc::MSG_REQ_FIELD_VALUE, &request);
    }

    /// Insert the armored public key of the local user into a field.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInsertPublicKey(
        mut env: JNIEnv,
        _this: JObject,
        node_id: jstring,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(frame) = browser.main_frame() else {
            return;
        };
        let Ok(node_id) = read_string(&mut env, node_id) else {
            return;
        };

        register_pending(
            &node_id,
            ov_cef::ipc::FieldAction::InsertPublicKey,
            Vec::new(),
        );

        let request = ov_cef::ipc::FieldValueRequest {
            node_id,
            action: ov_cef::ipc::FieldAction::InsertPublicKey,
            recipients: Vec::new(),
        };
        ov_cef::ipc::send_json_to_renderer(&frame, ov_cef::ipc::MSG_REQ_FIELD_VALUE, &request);
    }

    /// Register a pending field action.
    fn register_pending(node_id: &str, action: ov_cef::ipc::FieldAction, recipients: Vec<String>) {
        let Some(state) = engine() else {
            return;
        };
        let mut pending = state.pending.lock().unwrap();
        pending.insert(node_id.to_string(), (action, recipients));
    }

    /// Decrypt the armored message of a node.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDecryptNode(
        mut env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        armored: jstring,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let Some(browser) = state.browser() else {
            return;
        };
        let Some(frame) = browser.main_frame() else {
            return;
        };
        let (Ok(node_id), Ok(armored)) = (
            read_string(&mut env, node_id),
            read_string(&mut env, armored),
        ) else {
            return;
        };
        let request = ov_cef::ipc::DecryptRequest { node_id, armored };
        ov_cef::ipc::send_json(&frame, ov_cef::ipc::MSG_REQ_DECRYPT, &request);
    }

    /// Import an armored public key that was found in the page.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeImportKey(
        mut env: JNIEnv,
        _this: JObject,
        armored: jstring,
    ) {
        let Some(state) = engine() else {
            return;
        };
        let armored = match read_string(&mut env, armored) {
            Ok(value) => value,
            Err(err) => {
                log::error!("{err:#}");
                return;
            }
        };
        match state.keyring.import_cert(armored.as_bytes()) {
            Ok(fingerprint) => {
                native::call_native(
                    "showError",
                    &[native::NativeArg::String(format!(
                        "imported the key {fingerprint}"
                    ))],
                );
            }
            Err(err) => {
                native::call_native(
                    "showError",
                    &[native::NativeArg::String(format!(
                        "cannot import the key: {err:#}"
                    ))],
                );
            }
        }
    }

    /// Generate a new key pair.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGenerateKey(
        mut env: JNIEnv,
        _this: JObject,
        user_id: jstring,
    ) -> jstring {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let user_id = match read_string(&mut env, user_id) {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        match state.keyring.generate_keypair(&user_id) {
            Ok(fingerprint) => match env.new_string(fingerprint.to_string()) {
                Ok(value) => value.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            Err(err) => {
                native::call_native(
                    "showError",
                    &[native::NativeArg::String(format!(
                        "cannot generate the key: {err:#}"
                    ))],
                );
                std::ptr::null_mut()
            }
        }
    }

    /// Go forward in the history of the browser.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGoForward(
        _env: JNIEnv,
        _this: JObject,
    ) -> jboolean {
        let Some(state) = engine() else {
            return 0;
        };
        let Some(browser) = state.browser() else {
            return 0;
        };
        if !browser.can_go_forward().cef_truthy() {
            return 0;
        }
        browser.go_forward();
        1
    }

    /// The keys of the keyring.
    ///
    /// The list is a JSON document with one entry per certificate: the
    /// fingerprint, the user id, whether a secret key is stored for it and
    /// the moment of its creation.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeListKeys(
        env: JNIEnv,
        _this: JObject,
    ) -> jstring {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let certificates = match state.keyring.certificates() {
            Ok(certificates) => certificates,
            Err(err) => {
                log::error!("cannot read the keyring: {err:#}");
                return std::ptr::null_mut();
            }
        };

        let entries: Vec<serde_json::Value> = certificates
            .iter()
            .map(|certificate| {
                serde_json::json!({
                    "fingerprint": certificate.fingerprint.to_string(),
                    "userId": ov_pgp::user_id_of(&certificate.armored).unwrap_or_default(),
                    "hasSecret": certificate.has_secret,
                    "createdAt": certificate.created_at,
                })
            })
            .collect();
        let payload = serde_json::json!({ "keys": entries });

        match env.new_string(payload.to_string()) {
            Ok(value) => value.into_raw(),
            Err(err) => {
                log::error!("cannot create the key list: {err}");
                std::ptr::null_mut()
            }
        }
    }

    /// Export the armored public key of a fingerprint.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeExportPublicKey(
        mut env: JNIEnv,
        _this: JObject,
        fingerprint: jstring,
    ) -> jstring {
        let Some(state) = engine() else {
            return std::ptr::null_mut();
        };
        let fingerprint = match read_string(&mut env, fingerprint) {
            Ok(value) => value,
            Err(_) => return std::ptr::null_mut(),
        };
        let Ok(fingerprint) = ov_pgp::parse_fingerprint(&fingerprint) else {
            native::call_native(
                "showError",
                &[native::NativeArg::String(format!(
                    "{fingerprint} is not a fingerprint"
                ))],
            );
            return std::ptr::null_mut();
        };
        match state.keyring.export_armored_public_key(&fingerprint) {
            Ok(armored) => match env.new_string(armored) {
                Ok(value) => value.into_raw(),
                Err(_) => std::ptr::null_mut(),
            },
            Err(err) => {
                native::call_native(
                    "showError",
                    &[native::NativeArg::String(format!("{err:#}"))],
                );
                std::ptr::null_mut()
            }
        }
    }

    /// Run a CEF subprocess.
    ///
    /// The subprocesses are started by the Android services of
    /// `CefService.kt`; the command line is passed as a list of switches.
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeRunSubprocess(
        mut env: JNIEnv,
        _this: JObject,
        process_type: jstring,
        switches: jni::objects::JObjectArray<'static>,
    ) -> jint {
        let process_type = match read_string(&mut env, process_type) {
            Ok(value) => value,
            Err(_) => return -1,
        };
        let mut args: Vec<String> =
            vec!["onionvault".to_string(), format!("--type={process_type}")];
        let count = env.get_array_length(&switches).unwrap_or(0);
        for index in 0..count {
            let Ok(switch) = env.get_object_array_element(&switches, index) else {
                continue;
            };
            if let Ok(value) = env.get_string(&JString::from(switch)) {
                args.push(value.into());
            }
        }

        let mut owned: Vec<std::ffi::CString> = args
            .into_iter()
            .filter_map(|arg| std::ffi::CString::new(arg).ok())
            .collect();
        let mut pointers: Vec<*mut std::os::raw::c_char> = owned
            .iter_mut()
            .map(|value| value.as_ptr() as *mut std::os::raw::c_char)
            .collect();
        let args = cef::MainArgs {
            argc: pointers.len() as std::os::raw::c_int,
            argv: pointers.as_mut_ptr(),
        };

        let (app, _) = ov_cef::build_engine(
            Arc::new(OvConfig::default()),
            Arc::new(ov_cef::NullBridge),
            Arc::new(SurfaceRenderer::new(ov_cef::PixelFormat::Rgba8888)),
            Arc::new(BrowserSlot::new()),
            ov_cef::pending_actions(),
            Arc::new(ov_cef::SnapshotCollector::new()),
        );
        let mut app = app;
        cef::execute_process(Some(&args), Some(&mut app), std::ptr::null_mut())
    }
}
