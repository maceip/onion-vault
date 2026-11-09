//! Calls from the native engine into the Android UI.
//!
//! The Kotlin side exposes the callbacks as static methods of
//! `dev.onionvault.OvBridge`; the engine calls them through JNI.

#[cfg(target_os = "android")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "android")]
static JAVA_VM: OnceLock<Mutex<Option<usize>>> = OnceLock::new();

/// An argument of a call into the native UI.
pub enum NativeArg {
    /// A string argument.
    String(String),
    /// A boolean argument.
    Bool(bool),
    /// An integer argument.
    Int(i32),
}

/// Remember the Java VM so that the engine can call back into the UI.
#[cfg(target_os = "android")]
pub fn set_java_vm(vm: jni::JavaVM) {
    let slot = JAVA_VM.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = Some(vm.get_java_vm_pointer() as usize);
}

/// The Java VM of the application.
///
/// The Keystore needs it, and it can only be obtained through a JNI
/// environment; the environment hands it over when the engine starts.
#[cfg(target_os = "android")]
pub fn java_vm() -> Option<jni::JavaVM> {
    let pointer = java_vm_pointer()?;
    // The Java VM belongs to the application; the wrapper does not destroy
    // it when it is dropped.
    unsafe { jni::JavaVM::from_raw(pointer) }.ok()
}

/// The raw pointer of the Java VM.
#[cfg(target_os = "android")]
pub fn java_vm_pointer() -> Option<*mut jni::sys::JavaVM> {
    let slot = JAVA_VM.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap();
    let pointer = *guard.as_ref()?;
    Some(pointer as *mut jni::sys::JavaVM)
}

/// The raw `JNIEnv*` of the current thread.
///
/// The NDK needs the pointer of the JNI environment; cef-rs and the NDK
/// crate do not share a type for it, so it is read out of the Java VM.
#[cfg(target_os = "android")]
pub fn jni_env_pointer() -> Option<*mut jni::sys::JNIEnv> {
    let vm = java_vm_pointer()?;
    // The JNI interface of the virtual machine hands out the environment of
    // the current thread.
    let mut env: *mut std::ffi::c_void = std::ptr::null_mut();
    let get_env = unsafe { (*(*vm)).GetEnv? };
    let result = unsafe { get_env(vm, &mut env, jni::sys::JNI_VERSION_1_6) };
    if result != 0 || env.is_null() {
        return None;
    }
    Some(env as *mut jni::sys::JNIEnv)
}

/// Call a static method of `dev.onionvault.OvBridge`.
///
/// The supported methods are
///
///   * `onDomScan(String)`,
///   * `onNavigation(String)`,
///   * `showPlaintext(String, String, String, boolean)`,
///   * `onMutationResult(String)`,
///   * `showError(String)` and
///   * `onTorProgress(int, String)`.
#[cfg(target_os = "android")]
pub fn call_native(name: &str, args: &[NativeArg]) {
    let Some(vm) = java_vm() else {
        log::debug!("no Java VM, cannot call {name}");
        return;
    };

    let Ok(mut env) = vm.attach_current_thread() else {
        log::error!("cannot attach to the Java VM");
        return;
    };
    let Ok(class) = env.find_class("dev/onionvault/OvBridge") else {
        log::error!("cannot find dev.onionvault.OvBridge");
        return;
    };

    let result = match name {
        "onDomScan" => call_with_string(&mut env, &class, "onDomScan", "onDomScan", args),
        "onNavigation" => call_with_string(&mut env, &class, "onNavigation", "onNavigation", args),
        "showError" => call_with_string(&mut env, &class, "showError", "showError", args),
        "onMutationResult" => call_with_string(
            &mut env,
            &class,
            "onMutationResult",
            "onMutationResult",
            args,
        ),
        "showPlaintext" => {
            let strings = collect_strings(args);
            if strings.len() < 3 {
                log::error!("showPlaintext needs three strings");
                return;
            }
            let verified = matches!(args.get(3), Some(NativeArg::Bool(true)));
            let a = string_arg(&mut env, &safe(&strings[0]));
            let b = string_arg(&mut env, &safe(&strings[1]));
            let c = string_arg(&mut env, &safe(&strings[2]));
            env.call_static_method(
                &class,
                "showPlaintext",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Z)V",
                &[
                    (&a).into(),
                    (&b).into(),
                    (&c).into(),
                    (verified as u8).into(),
                ],
            )
            .map(|_| ())
        }
        "onTorProgress" => {
            let percent = match args.first() {
                Some(NativeArg::Int(value)) => *value,
                _ => 0,
            };
            let stage = match args.get(1) {
                Some(NativeArg::String(value)) => value.clone(),
                _ => String::new(),
            };
            let stage = string_arg(&mut env, &stage);
            env.call_static_method(
                &class,
                "onTorProgress",
                "(ILjava/lang/String;)V",
                &[(percent as i32).into(), (&stage).into()],
            )
            .map(|_| ())
        }
        other => {
            log::warn!("unknown native callback {other}");
            return;
        }
    };

    if let Err(err) = result {
        log::error!("the native callback {name} failed: {err}");
    }
}

/// Create the string argument of a call into the native UI.
#[cfg(target_os = "android")]
fn string_arg<'local>(env: &mut jni::JNIEnv<'local>, value: &str) -> jni::objects::JString<'local> {
    match env.new_string(value) {
        Ok(value) => value,
        Err(err) => {
            log::error!("cannot create the string argument: {err}");
            env.new_string("")
                .expect("the JVM cannot even create an empty string")
        }
    }
}

/// Call a static method that takes a single string.
#[cfg(target_os = "android")]
fn call_with_string(
    env: &mut jni::JNIEnv,
    class: &jni::objects::JClass<'_>,
    method: &str,
    _label: &str,
    args: &[NativeArg],
) -> jni::errors::Result<()> {
    let value = match args.first() {
        Some(NativeArg::String(value)) => value.clone(),
        _ => String::new(),
    };
    let value = string_arg(env, &value);
    env.call_static_method(class, method, "(Ljava/lang/String;)V", &[(&value).into()])?;
    Ok(())
}

/// Collect the string arguments of a call.
#[cfg(target_os = "android")]
fn collect_strings(args: &[NativeArg]) -> Vec<String> {
    args.iter()
        .filter_map(|arg| match arg {
            NativeArg::String(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

/// Replace invalid characters so that the JNI conversion cannot fail.
#[cfg(target_os = "android")]
fn safe(value: &str) -> String {
    value.replace('\u{0}', "")
}

/// The host build has no UI; the calls are ignored.
#[cfg(not(target_os = "android"))]
pub fn call_native(name: &str, _args: &[NativeArg]) {
    log::debug!("ignoring the native callback {name}");
}
