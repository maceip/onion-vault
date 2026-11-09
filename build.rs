//! Build script for the `onion-vault` Android library.
//!
//! Two jobs:
//!
//! 1. **Link the prebuilt CEF runtime.**  CEF for Android is distributed as a
//!    prebuilt `libcef.so` plus a cross-compiled C++ wrapper
//!    (`libcef_dll_wrapper.a`).  Neither can be produced by Cargo, so the
//!    sysroot is located through `CEF_ANDROID_ROOT` (falling back to
//!    `CEF_PATH`) and the link directives are emitted from here.  The
//!    vendored `cef-dll-sys` build script performs the same lookup, this one
//!    additionally covers the `onion-vault` cdylib itself.
//!
//! 2. **Package the CEF resources.**  `cef.pak`, `cef_100_percent.pak`,
//!    `icudtl.dat` and `locales/en-US.pak` have to be shipped inside the APK
//!    (`assets/cef/`) and unpacked to `filesDir/cef_resources` on first boot,
//!    because Android's asset manager cannot hand Chromium a memory mapped
//!    file descriptor for them.  This script copies them into the Gradle
//!    assets directory so that `./gradlew assembleDebug` picks them up.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Resources that must end up in `android/app/src/main/assets/cef/`.
const CEF_RESOURCES: &[&str] = &[
    "cef.pak",
    "cef_100_percent.pak",
    "cef_200_percent.pak",
    "icudtl.dat",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    // The Chromium names of the resources.  A CEF distribution uses the
    // `cef_*` names, a plain Chromium build uses the `chrome_*` names and
    // `resources.pak`; the engine reads the names it was built with, so
    // every name that exists is copied.
    "resources.pak",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CEF_ANDROID_ROOT");
    println!("cargo::rerun-if-env-changed=CEF_PATH");
    println!("cargo::rerun-if-env-changed=ONIONVAULT_CEF_CHECK_ONLY");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets_dir = manifest_dir.join("android/app/src/main/assets/cef");

    let Some(root) = cef_root() else {
        println!(
            "cargo::warning=CEF_ANDROID_ROOT is not set: the CEF runtime is neither linked nor \
             packaged.  Set CEF_ANDROID_ROOT to a CEF Android distribution to build an APK."
        );
        return;
    };

    link_cef(&root);
    package_resources(&root, &assets_dir);
}

/// Root of the pre-extracted CEF distribution.
fn cef_root() -> Option<PathBuf> {
    env::var("CEF_ANDROID_ROOT")
        .ok()
        .or_else(|| env::var("CEF_PATH").ok())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

/// Emit the link directives for the prebuilt CEF shared object.
fn link_cef(root: &Path) {
    let lib_dir = root.join("lib");
    let libcef = lib_dir.join("libcef.so");
    if libcef.is_file() {
        println!("cargo::rustc-link-search=native={}", lib_dir.display());
        println!("cargo::rustc-link-lib=dylib=cef");
        println!("cargo::rerun-if-changed={}", libcef.display());
    } else {
        println!(
            "cargo::warning={} not found: libcef.so will not be linked",
            libcef.display()
        );
    }

    // The C++ wrapper static library, as produced by the NDK toolchain build
    // of the CEF `libcef_dll` wrapper.
    for candidate in [
        root.join("libcef_dll_wrapper.a"),
        root.join("libcef_dll_wrapper/libcef_dll_wrapper.a"),
        root.join("build/libcef_dll_wrapper.a"),
    ] {
        if candidate.is_file() {
            let dir = candidate.parent().unwrap();
            println!("cargo::rustc-link-search=native={}", dir.display());
            println!("cargo::rustc-link-lib=static=cef_dll_wrapper");
            println!("cargo::rerun-if-changed={}", candidate.display());
            break;
        }
    }
}

/// Copy the CEF resource files into the Gradle assets directory.
fn package_resources(root: &Path, assets_dir: &Path) {
    if let Err(err) = fs::create_dir_all(assets_dir) {
        println!(
            "cargo::warning=cannot create {}: {err}",
            assets_dir.display()
        );
        return;
    }

    let mut copied = 0usize;
    for name in CEF_RESOURCES {
        let src = root.join(name);
        if src.is_file() {
            if let Err(err) = fs::copy(&src, assets_dir.join(name)) {
                println!("cargo::warning=cannot copy {}: {err}", src.display());
            } else {
                copied += 1;
            }
        }
    }

    // Locales live in a separate directory of the distribution.
    let locales_src = root.join("locales");
    let locales_dst = assets_dir.join("locales");
    if locales_src.is_dir() {
        if let Err(err) = fs::create_dir_all(&locales_dst) {
            println!(
                "cargo::warning=cannot create {}: {err}",
                locales_dst.display()
            );
        } else if let Ok(entries) = fs::read_dir(&locales_src) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = fs::copy(&path, locales_dst.join(entry.file_name()));
                    copied += 1;
                }
            }
        }
    }

    if copied == 0 {
        println!(
            "cargo::warning=no CEF resource files found below {}: the APK would not start CEF",
            root.display()
        );
    } else {
        println!("cargo::warning=packaged {copied} CEF resource file(s) into the APK assets");
    }
}
