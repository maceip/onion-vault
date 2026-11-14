//! Trap: a zero-JavaScript mobile browser for Android.
//!
//! The crate is the shared library of the Android application; it is loaded
//! by `dev.onionvault.MainActivity` and by the CEF subprocess services.
//!
//! The native code lives in the four workspace crates:
//!
//!   * [`ov_tor`] runs the embedded Arti Tor client and the local SOCKS5
//!     listener that CEF is pointed at,
//!   * [`ov_cef`] runs the off-screen rendered CEF browser, the native DOM
//!     PGP engine and the snapshot of the visited pages,
//!   * [`ov_pgp`] runs the hardware backed OpenPGP key manager, and
//!   * [`ov_core`] owns the Android specifics: the JNI implementation, the
//!     native window and the translation of the Android input events.
//!
//! The JNI symbols are exported here: the shared library of the application
//! is the only object that the Android runtime loads, so the entry points
//! have to live in it.  They forward to the implementation in `ov_core`.

pub use ov_core;

/// The version of the native library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The JNI entry points of the native library.
///
/// The functions only convert the JNI types into the Rust types of
/// `ov_core` and call the implementation there.
#[cfg(target_os = "android")]
mod jni_entry_points {
    use jni::objects::JObject;
    use jni::sys::{jboolean, jdouble, jint, jstring};
    use jni::JNIEnv;

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeInit`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInit(
        env: JNIEnv,
        _this: JObject,
        files_dir: jstring,
        _cache_dir: jstring,
        hidden_service_only: jboolean,
        require_auth: jboolean,
    ) -> jboolean {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeInit(
            env,
            _this,
            files_dir,
            _cache_dir,
            hidden_service_only,
            require_auth,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeShutdown`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeShutdown(
        _env: JNIEnv,
        _this: JObject,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeShutdown(_env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeCreateSurface`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeCreateSurface(
        _env: JNIEnv,
        _this: JObject,
        surface: JObject,
        width: jint,
        height: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeCreateSurface(
            _env, _this, surface, width, height,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeDestroySurface`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDestroySurface(
        _env: JNIEnv,
        _this: JObject,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeDestroySurface(_env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeResize`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeResize(
        _env: JNIEnv,
        _this: JObject,
        width: jint,
        height: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeResize(_env, _this, width, height)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeDoMessageLoopWork`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDoMessageLoopWork(
        _env: JNIEnv,
        _this: JObject,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeDoMessageLoopWork(_env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeLoadUrl`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeLoadUrl(
        env: JNIEnv,
        _this: JObject,
        url: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeLoadUrl(env, _this, url)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSnapshotCount`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotCount(
        _env: JNIEnv,
        _this: JObject,
    ) -> jint {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSnapshotCount(_env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSnapshotInfo`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotInfo(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jstring {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSnapshotInfo(env, _this, index)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSnapshotFrame`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotFrame(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jni::sys::jbyteArray {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSnapshotFrame(env, _this, index)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSnapshotMarkup`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSnapshotMarkup(
        env: JNIEnv,
        _this: JObject,
        index: jint,
    ) -> jni::sys::jbyteArray {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSnapshotMarkup(env, _this, index)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeGoBack`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGoBack(
        _env: JNIEnv,
        _this: JObject,
    ) -> jboolean {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeGoBack(_env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeTouch`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeTouch(
        _env: JNIEnv,
        _this: JObject,
        action: jint,
        x: f32,
        y: f32,
        id: jint,
        modifiers: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeTouch(
            _env, _this, action, x, y, id, modifiers,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeMouseButton`.
    #[no_mangle]
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
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeMouseButton(
            _env,
            _this,
            x,
            y,
            button,
            mouse_up,
            modifiers,
            click_count,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeMouseMove`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeMouseMove(
        _env: JNIEnv,
        _this: JObject,
        x: f32,
        y: f32,
        modifiers: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeMouseMove(_env, _this, x, y, modifiers)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeMouseWheel`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeMouseWheel(
        _env: JNIEnv,
        _this: JObject,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        modifiers: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeMouseWheel(
            _env, _this, x, y, delta_x, delta_y, modifiers,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeKey`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeKey(
        _env: JNIEnv,
        _this: JObject,
        action: jint,
        key_code: jint,
        character: jint,
        modifiers: jint,
        is_system_key: jint,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeKey(
            _env,
            _this,
            action,
            key_code,
            character,
            modifiers,
            is_system_key,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeImeCommitText`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeImeCommitText(
        env: JNIEnv,
        _this: JObject,
        text: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeImeCommitText(env, _this, text)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeInsertPlaintext`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInsertPlaintext(
        env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        text: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeInsertPlaintext(
            env, _this, node_id, text,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeEncryptField`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeEncryptField(
        env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        recipients: jni::objects::JObjectArray<'static>,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeEncryptField(
            env, _this, node_id, recipients,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeInsertPublicKey`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeInsertPublicKey(
        env: JNIEnv,
        _this: JObject,
        node_id: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeInsertPublicKey(env, _this, node_id)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeDecryptNode`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeDecryptNode(
        env: JNIEnv,
        _this: JObject,
        node_id: jstring,
        armored: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeDecryptNode(
            env, _this, node_id, armored,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeImportKey`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeImportKey(
        env: JNIEnv,
        _this: JObject,
        armored: jstring,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeImportKey(env, _this, armored)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeGenerateKey`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGenerateKey(
        env: JNIEnv,
        _this: JObject,
        user_id: jstring,
    ) -> jstring {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeGenerateKey(env, _this, user_id)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeGoForward`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeGoForward(
        env: JNIEnv,
        _this: JObject,
    ) -> jboolean {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeGoForward(env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSetZoom`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSetZoom(
        env: JNIEnv,
        _this: JObject,
        level: jdouble,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSetZoom(env, _this, level)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeSetHiddenServiceOnly`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeSetHiddenServiceOnly(
        env: JNIEnv,
        _this: JObject,
        hidden_service_only: jboolean,
    ) {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeSetHiddenServiceOnly(
            env,
            _this,
            hidden_service_only,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeListKeys`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeListKeys(
        env: JNIEnv,
        _this: JObject,
    ) -> jstring {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeListKeys(env, _this)
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeExportPublicKey`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeExportPublicKey(
        env: JNIEnv,
        _this: JObject,
        fingerprint: jstring,
    ) -> jstring {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeExportPublicKey(
            env,
            _this,
            fingerprint,
        )
    }

    /// The JNI entry point `Java_dev_onionvault_OvNative_nativeRunSubprocess`.
    #[no_mangle]
    pub extern "system" fn Java_dev_onionvault_OvNative_nativeRunSubprocess(
        env: JNIEnv,
        _this: JObject,
        process_type: jstring,
        switches: jni::objects::JObjectArray<'static>,
    ) -> jint {
        ov_core::jni_api::Java_dev_onionvault_OvNative_nativeRunSubprocess(
            env,
            _this,
            process_type,
            switches,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_version_is_not_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
