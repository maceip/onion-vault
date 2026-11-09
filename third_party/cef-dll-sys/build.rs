//! Build script for the vendored, Android-patched `cef-dll-sys` crate.
//!
//! Upstream `cef-dll-sys` (https://github.com/tauri-apps/cef-rs) resolves a CEF
//! *desktop* binary distribution for the host triple, compiles the C++ wrapper
//! (`libcef_dll_wrapper`) with CMake and links `libcef`.
//!
//! OnionVault targets `aarch64-linux-android`, which upstream does not support:
//! there is no `cef_linux_aarch64` distribution for the Android ABI, the
//! wrapper has to be cross-compiled with the NDK toolchain, and `libcef.so`
//! must end up inside the APK instead of next to the host binary.
//!
//! This build script therefore keeps the upstream behaviour for desktop
//! targets and adds an Android path that
//!
//!   * consumes a pre-extracted CEF *Android* sysroot pointed at by
//!     `CEF_ANDROID_ROOT` (or `CEF_PATH`),
//!   * links the prebuilt `libcef.so` plus the cross-compiled
//!     `libcef_dll_wrapper.a` found in that sysroot, and
//!   * can be switched into "check only" mode (`ONIONVAULT_CEF_CHECK_ONLY=1`)
//!     in which no CEF distribution is required at all.  `cargo check` never
//!     links, so this keeps the type check of the whole workspace usable on a
//!     machine without the (multi gigabyte) CEF Android distribution.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// `true` when the build must not require a CEF distribution.
fn check_only() -> bool {
    env::var("ONIONVAULT_CEF_CHECK_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// `true` when the target triple is an Android ABI.
fn is_android(target: &str) -> bool {
    target.contains("android")
}

/// Root of the pre-extracted CEF distribution for the current build.
fn cef_root() -> Option<PathBuf> {
    env::var("CEF_ANDROID_ROOT")
        .ok()
        .or_else(|| env::var("CEF_PATH").ok())
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

fn main() -> anyhow::Result<()> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CEF_ANDROID_ROOT");
    println!("cargo::rerun-if-env-changed=CEF_PATH");
    println!("cargo::rerun-if-env-changed=ONIONVAULT_CEF_CHECK_ONLY");
    println!("cargo::rerun-if-env-changed=FLATPAK");
    println!("cargo::rerun-if-env-changed=NIX_CEF_BINARY");

    let target = env::var("TARGET")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    if is_android(&target) {
        return build_android(&target, &out_dir);
    }

    if check_only() {
        // Desktop check-only mode: no distribution, no wrapper, no link step.
        return Ok(());
    }

    desktop_build(&target, &out_dir)
}

/// Android build: link against a pre-extracted CEF Android sysroot.
fn build_android(_target: &str, out_dir: &Path) -> anyhow::Result<()> {
    let Some(root) = cef_root() else {
        if check_only() {
            println!(
                "cargo::warning=ONIONVAULT_CEF_CHECK_ONLY=1: skipping the CEF link step; \
                 set CEF_ANDROID_ROOT to a CEF Android distribution to build a binary"
            );
            return Ok(());
        }
        anyhow::bail!(
            "no CEF Android distribution found.\n\
             Set CEF_ANDROID_ROOT to a directory that contains the CEF Android SDK, e.g.\n    \
             export CEF_ANDROID_ROOT=$HOME/cef/android-arm64\n\
             It must provide libcef.so, the C++ wrapper static library and the CEF headers."
        );
    };

    let lib_dir = root.join("lib");
    let wrapper_lib_dir = root.join("libcef_dll_wrapper");

    // The CEF Android SDK ships the shared object directly in `lib/`.
    let libcef = lib_dir.join("libcef.so");
    if !libcef.exists() {
        anyhow::bail!("{} does not exist", libcef.display());
    }
    println!("cargo::rerun-if-changed={}", libcef.display());
    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    println!("cargo::rustc-link-lib=dylib=cef");

    // The C++ wrapper (`libcef_dll_wrapper.a`) is the shim that implements the
    // `cef_*` C API on top of the C++ API.  It is cross-compiled with the NDK
    // toolchain; OnionVault accepts a prebuilt copy inside the sysroot.
    let wrapper = find_wrapper_archive(&wrapper_lib_dir, &root);
    match wrapper {
        Some(archive) => {
            let dir = archive.parent().unwrap_or(&root).to_path_buf();
            println!("cargo::rustc-link-search=native={}", dir.display());
            println!("cargo::rustc-link-lib=static=cef_dll_wrapper");
            println!("cargo::rerun-if-changed={}", archive.display());
        }
        None if check_only() => {
            println!(
                "cargo::warning=ONIONVAULT_CEF_CHECK_ONLY=1: {} has no libcef_dll_wrapper.a; \
                 the link step is skipped",
                root.display()
            );
        }
        None => {
            // A CEF Android distribution can ship the C++ wrapper as a
            // separate static archive (`libcef_dll_wrapper.a`) or as part of
            // the shared object itself.  The second form is complete on its
            // own: every C API function is exported by `libcef.so`.
            println!(
                "cargo::warning={} has no libcef_dll_wrapper.a: the shared object is expected \
                 to contain the C++ wrapper as well",
                root.display()
            );
        }
    }

    // Expose the resource directory so the app can hand it to
    // `CefSettings.resources_dir_path` / `locales_dir_path`.
    println!("cargo::metadata=CEF_ANDROID_ROOT={}", root.display());
    let _ = out_dir;
    Ok(())
}

/// Locate `libcef_dll_wrapper.a` below the sysroot.
fn find_wrapper_archive(dir: &Path, root: &Path) -> Option<PathBuf> {
    let direct = dir.join("libcef_dll_wrapper.a");
    if direct.is_file() {
        return Some(direct);
    }
    let nested = root.join("libcef_dll_wrapper.a");
    if nested.is_file() {
        return Some(nested);
    }
    // `build/libcef_dll_wrapper.a` is where the upstream CMake build leaves it.
    let cmake = root.join("build/libcef_dll_wrapper.a");
    if cmake.is_file() {
        return Some(cmake);
    }
    None
}

/// Desktop build: unchanged upstream behaviour (download + CMake + Ninja).
fn desktop_build(target: &str, out_dir: &Path) -> anyhow::Result<()> {
    use download_cef::OsAndArch;

    let os_arch = OsAndArch::try_from(target)?;
    let package_version = env::var("CARGO_PKG_VERSION")?;
    let cef_version = download_cef::default_version(&package_version);

    let check_archive = |path: &Path| -> anyhow::Result<()> {
        download_cef::check_archive_json(&package_version, &path.to_string_lossy())?;
        Ok(())
    };

    let resolve_cef_dir = |location: &Path| -> anyhow::Result<PathBuf> {
        let cef_dir = location.join(os_arch.to_string());
        if !fs::exists(&cef_dir)? {
            use download_cef::CefIndex;
            let download_url = download_cef::default_download_url();
            let index = CefIndex::download_from(&download_url)?;
            let platform = index.platform(target)?;
            let version = platform.version(&cef_version)?;
            let archive = version.download_archive_from(&download_url, location, false)?;
            let extracted_dir = download_cef::extract_target_archive(target, &archive, location, false)?;
            let extracted_dir_canonical = fs::canonicalize(&extracted_dir)?;
            let cef_dir_canonical = fs::canonicalize(&cef_dir)?;
            if extracted_dir_canonical != cef_dir_canonical {
                anyhow::bail!(
                    "extracted dir {extracted_dir_canonical:?} does not match cef_dir {cef_dir_canonical:?}"
                );
            }
            version.write_archive_json(extracted_dir)?;
        }
        Ok(cef_dir)
    };

    let cef_dir = if let Ok(cef_path) = env::var("CEF_PATH") {
        let configured_path = PathBuf::from(cef_path);
        if fs::exists(&configured_path)? {
            let versioned = configured_path.join(&cef_version);
            if fs::exists(&versioned)? {
                resolve_cef_dir(&versioned)?
            } else {
                match check_archive(&configured_path) {
                    Ok(()) => configured_path,
                    Err(_) => resolve_cef_dir(&versioned)?,
                }
            }
        } else {
            resolve_cef_dir(&configured_path)?
        }
    } else {
        resolve_cef_dir(out_dir)?
    };

    let cef_dir_str = cef_dir.to_string_lossy().into_owned();
    println!("cargo::rerun-if-changed={cef_dir_str}");
    println!("cargo::metadata=CEF_DIR={cef_dir_str}");
    println!("cargo::rustc-link-search=native={cef_dir_str}");

    let api_version = cef_api_version_last(&cef_dir)?;
    println!("cargo::metadata=CEF_API_VERSION={api_version}");

    let mut cef_dll_wrapper = cmake::Config::new(&cef_dir);
    cef_dll_wrapper
        .generator("Ninja")
        .profile("RelWithDebInfo")
        .build_target("libcef_dll_wrapper")
        .define("CEF_COMPILER_DEFINES", format!("CEF_API_VERSION={api_version}"));

    let project_arch = match os_arch.arch {
        "aarch64" => "arm64",
        arch => arch,
    };
    let sandbox = if cfg!(feature = "sandbox") { "ON" } else { "OFF" };

    match os_arch.os {
        "linux" | "windows" => {
            copy_cef_runtime_files(&cef_dir, out_dir)?;
            if os_arch.os == "windows" {
                for lib in [
                    "comctl32", "delayimp", "mincore", "powrprof", "propsys", "runtimeobject",
                    "setupapi", "shcore", "shell32", "shlwapi", "user32", "version", "wbemuuid",
                    "winmm",
                ] {
                    println!("cargo::rustc-link-lib=dylib={lib}");
                }
                cef_dll_wrapper
                    .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded")
                    .define("CMAKE_OBJECT_PATH_MAX", "500")
                    .define("PROJECT_ARCH", project_arch)
                    .define("USE_SANDBOX", sandbox);
            }
            let build_dir = cef_dll_wrapper.build().to_string_lossy().into_owned();
            println!("cargo::rustc-link-search=native={build_dir}/build/libcef_dll_wrapper");
            println!("cargo::rustc-link-lib=static=libcef_dll_wrapper");
            if os_arch.os == "linux" {
                println!("cargo::rustc-link-lib=dylib=cef");
            } else {
                println!("cargo::rustc-link-lib=dylib=libcef");
            }
        }
        "macos" => {
            println!("cargo::rustc-link-lib=framework=AppKit");
            let build_dir = cef_dll_wrapper
                .no_default_flags(true)
                .define("PROJECT_ARCH", project_arch)
                .define("USE_SANDBOX", sandbox)
                .build()
                .to_string_lossy()
                .into_owned();
            println!("cargo::rustc-link-search=native={build_dir}/build/libcef_dll_wrapper");
            println!("cargo::rustc-link-lib=static=cef_dll_wrapper");
        }
        os => anyhow::bail!("unknown target {os}"),
    }

    Ok(())
}

/// `CEF_API_VERSION_LAST` from the distribution's `include/cef_api_versions.h`.
fn cef_api_version_last(cef_dir: &Path) -> anyhow::Result<u32> {
    let header = cef_dir.join("include").join("cef_api_versions.h");
    let contents = fs::read_to_string(&header)?;
    contents
        .lines()
        .find_map(|line| {
            line.strip_prefix("#define CEF_API_VERSION_LAST ")?
                .trim()
                .strip_prefix("CEF_API_VERSION_")?
                .parse()
                .ok()
        })
        .ok_or_else(|| anyhow::anyhow!("no CEF_API_VERSION_LAST in {}", header.display()))
}

fn copy_directory(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let target = dest.join(entry.file_name());
            if target.is_file() {
                fs::remove_file(&target)?;
            }
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn copy_cef_runtime_files(cef_dir: &Path, target_dir: &Path) -> Result<(), std::io::Error> {
    copy_directory(cef_dir, target_dir)?;
    copy_directory(&cef_dir.join("locales"), &target_dir.join("locales"))
}
