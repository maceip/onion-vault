# OnionVault

[![CI & Release](https://github.com/maceip/onion-vault/actions/workflows/ci-release.yml/badge.svg)](https://github.com/maceip/onion-vault/actions/workflows/ci-release.yml)
[![Licence](https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)

OnionVault is an Android browser that routes every request through an
embedded Tor client, runs with **JavaScript switched off completely** and
implements OpenPGP natively in Rust — no WebAssembly, no content scripts, no
JavaScript at all.

The application is a Cargo workspace plus an Android/Gradle project:

```
onion-vault/
├── Cargo.toml              workspace + the `cdylib` of the application
├── build.rs                links the prebuilt CEF runtime, unpacks its resources
├── crates/
│   ├── ov-tor/             embedded Arti Tor client + local SOCKS5 listener
│   ├── ov-cef/             off-screen CEF browser + native DOM PGP engine
│   ├── ov-pgp/             Sequoia based key manager, Keystore wrapped TSKs
│   └── ov-core/            JNI entry points, ANativeWindow renderer, events
├── third_party/
│   ├── cef/                cef-rs, patched for aarch64-linux-android
│   └── cef-dll-sys/        cef-dll-sys, patched for aarch64-linux-android
├── src/lib.rs              the shared library that the APK packages
└── android/                the Gradle project (Kotlin, package `dev.onionvault`)
```

## Downloads

Every push of the pipeline publishes a tagged release with the debug APK and
with the full text of every licence that applies to a component.  The links
below always point at the newest build:

| file | what it is |
|---|---|
| [`app-debug.apk`](https://github.com/maceip/onion-vault/releases/latest/download/app-debug.apk) | the debug APK of the newest build (arm64-v8a, API 29 and later) |
| [`LGPL-3.0.txt`](https://github.com/maceip/onion-vault/releases/latest/download/LGPL-3.0.txt) | the licence of the CEF engine, with the GPL-3.0 that it incorporates |
| [`GPL-3.0.txt`](https://github.com/maceip/onion-vault/releases/latest/download/GPL-3.0.txt) | the GNU GPL on its own |
| [`LGPL-2.1.txt`](https://github.com/maceip/onion-vault/releases/latest/download/LGPL-2.1.txt) | the licence of sequoia-openpgp |
| [`CEFRIUM-NOTICE.txt`](https://github.com/maceip/onion-vault/releases/latest/download/CEFRIUM-NOTICE.txt) | the notice of the engine, with the licences of the Chromium sources |
| [`SHA256SUMS.txt`](https://github.com/maceip/onion-vault/releases/latest/download/SHA256SUMS.txt) | the checksums of the licence documents |

The list of every release is at
<https://github.com/maceip/onion-vault/releases>; the checksum of the APK is
written into the notes of each release.

The licences are inside the APK as well, under `assets/licenses/`, where the
application shows them: the button of the book in the top bar opens the
list of the documents.

## Security properties

| property | how it is enforced |
|---|---|
| **No JavaScript** | `CefBrowserSettings.javascript = State::DISABLED` for every browser; `javascript_close_windows`, `javascript_access_clipboard` and `javascript_dom_paste` are disabled as well. `CefFrame::execute_javascript` is never called anywhere in the tree. |
| **No clearnet leaks** | `--proxy-server=socks5://127.0.0.1:<port>`, `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1`, `--disable-webrtc`, `--incognito`, `--disable-features=NetworkPrediction`, `--disable-http2`, `--disable-background-networking`, `--no-first-run`. In strict hidden service mode every non-`.onion` request is cancelled and the `User-Agent`, `Accept-Language` and `Referer` headers are stripped. |
| **Secret keys never touch the disk in the clear** | Transferable secret keys are encrypted with an AES-256-GCM master key that lives inside the Android `KeyStore` (`KeyGenParameterSpec`, GCM, 256 bit, optional user authentication) before they are written into the SQLite keyring. The key is unlocked by the biometric prompt of the native UI for a short moment (`setUserAuthenticationValidityDurationSeconds`), and the keyring itself is excluded from the cloud backup and from the device transfer. |
| **The render process never sees a private key** | All cryptographic operations run in the browser process; the sandboxed renderer only receives the resulting text. |
| **No CSS side channel** | A decrypted message is shown in a native sheet, never inside the page. The plaintext is only written into the document when the user explicitly asks for it. |

## The Tor transport

The Tor client is part of the application: `arti-client` bootstraps inside the
sandbox of the app, the state and the cache live in the private directory of
the app and nothing is shelled out to an external `tor` process.

A SOCKS5 listener on `127.0.0.1:<ephemeral>` exposes the client to the CEF
network stack; Chromium is pointed at it with `--proxy-server`.  The listener
refuses every destination that is not a hidden service when the strict hidden
service mode is on, and it derives the circuit of a request from the first 16
base32 characters of the `.onion` name, so that two hidden services never
share a circuit.

rustls does not carry a cryptography provider of its own; the provider is
installed by `ov_tor::install_crypto_provider()` before the first client is
built, because the Arti client panics without one.

## The native DOM PGP engine

Because JavaScript is disabled, the PGP functionality cannot be implemented
with an injected script. Instead the render process walks Blink's DOM through
the CEF DOM API (`CefDOMVisitor` / `CefDOMNode`):

* form controls that can hold a PGP message are marked with a synthetic
  `data-ov-node-id` attribute and highlighted with an inline style,
* blocks that contain an armored PGP message or an armored public key are
  detected the same way,
* the bounding rectangles are reported to the browser process, which anchors
  the native action chips on them.

Every DOM change is an inline attribute mutation or a write through the DOM
API of CEF — no script is evaluated.

### Writing into a field

The DOM API of CEF is deliberately narrow, and its exact behaviour decides
whether a form sends the right text:

* `CefDOMNode::SetValue` **refuses element nodes** (`if (node_.IsElementNode())
  return false;` in CEF's own `libcef_domnode.cc`); it only replaces the text
  of a *text node*.
* `CefDOMNode::GetValue` returns the **live** value of a form control, but
  the empty string for every other element, and it trims the leading
  whitespace.
* An attribute write (`set_element_attribute("value", …)`) only reaches the
  live value of a control that the user has **not** typed into.  A control
  with a dirty value keeps the text of the user, and a form would post it.

OnionVault therefore writes a field in three steps and never trusts the
write:

1. the value is written — the `value` attribute of a control, the text node of
   a block (the text of a block is collected from its text nodes, because
   `GetValue` is empty for it),
2. the **live** value is read back and compared with the intended one,
3. when the comparison fails, the browser process finishes the job through
   the **input path** of the browser (`select_all`, `del` and the key events
   of the text — the same path a hardware keyboard uses) and the render
   process reads the value back a second time to confirm it.

The outcome is reported to the native UI in every case: a field that still
holds the old text would send it, and the user has to know.

## The snapshot back stack

A hidden service takes seconds to answer, sometimes minutes.  A back stack
that asks the network for every step is unusable, so OnionVault keeps the
pages it has already seen.

`OnionBackStackManager` (Kotlin) holds at most **12** entries.  Each entry
carries

| field | source |
|---|---|
| `url` | the load handler of the browser process |
| `title` | the DOM scan of the render process |
| `scrollOffset` | `on_scroll_offset_changed` |
| `frameSnapshot` | the off-screen frame of `on_paint`, reduced by four and swizzled to RGBA |
| `domHtmlSnapshot` | the markup of the document after the mutations, at most 512 KiB |
| `pgpFieldOverlays` | the PGP fields of the page, with their position and their value |

The frames are too large for a callback, so the UI *pulls* them: the browser
process asks the engine for the number of entries, for the meta data of an
entry, for its frame and for its markup (`nativeSnapshotCount`,
`nativeSnapshotInfo`, `nativeSnapshotFrame`, `nativeSnapshotMarkup`).

While the user swipes back, `PredictiveBackHandler` shrinks the live surface
with an `Animatable` and shows the cached frame of the page behind it.  The
gesture can be cancelled, the live surface then grows back to its full size.
The chrome of the page offers a *Reload Live over Tor* chip: the snapshot is
shown instantly, the real page is fetched in the background.

The manifest sets `android:enableOnBackInvokedCallback="true"`, so the
predictive back gesture is handed to the application instead of the system.

## The identity of OnionVault

The launcher icon and the whole interface are painted with the artwork of
the application: a cartoon onion on a badge that fades from teal at the top
left to cyan at the bottom right.

| colour | value | where it is used |
|---|---|---|
| teal | `#1DE5C2` | the primary colour: the navigation, the decryption, the focus |
| cyan | `#10A1D8` | the end of the gradient of the progress bar |
| amber | `#F5A93C` | the secondary colour: the primary button, the forward and the reload |
| leaf | `#A9E45F` | the accent: the keys |
| outline | `#3A1608` | the line of the artwork, the text on the amber |
| surface | `#14110E` | the background of the application |

The icon is installed as the legacy icon (a square and a round variant for
every density) and as the adaptive icon: the background is the gradient of
the badge, the foreground is the onion of the artwork, and the monochrome
layer is its silhouette for the themed icons.

Every control of the browser is drawn for this application, in
`res/drawable/ic_ov_*.xml`: the chevrons of the back and the forward
button, the circular arrow of the reload button, the window with a plus of
the new tab button, the key with a leaf of the new key button, the key on a
ring of the key manager, the shield of the transport, the padlocks of the
encryption and the decryption, and the arrow into a page that writes the
plaintext into the page.

The screen has two bars: the brand bar with the mark of the application, the
title of the page, the state of the transport and the key manager, and the
navigation bar with the back, the forward, the reload and the new tab
button, the address field and the *Go* button.

## The user interface

The user interface is Jetpack Compose.  Every screen is a pure function of
immutable state:

| class | content |
|---|---|
| `BrowserUiState` | the address, the page state, the fields, the PGP overlays |
| `TorBootstrapState` | the bootstrap, the progress, the circuit and its hops |
| `PgpOverlayState` | the PGP fields, the sheet with the decrypted plaintext |

The state has no reference to a JNI pointer or to a CEF object, so the
previews of `ui/previews/OnionVaultPreviews.kt` render the real screens in
Android Studio and on the JVM:

| preview | shows |
|---|---|
| `TorBootstrapPreview` | the bootstrap and the circuit at 65 % |
| `PgpChipsPreview` | a form without JavaScript and the PGP chips on it |
| `PredictiveBackPreview` | the back gesture at 65 % of its length |
| `PgpKeyManagerPreview` | the key manager and the decryption dialog |

`OnionVaultPreviewsTest` renders all four previews with Robolectric, which
runs the real Compose runtime on the JVM; the continuous integration runs it
on every push.

`ScreenshotTest` writes the same previews as PNG files into
`build/screenshots/`, so that the design can be inspected without a device:
it draws the window of the activity and the window of the dialog of the key
manager into the image.

## The IPC

The render process and the browser process talk through `CefProcessMessage`s
whose first argument is a JSON document:

| message | direction | payload |
|---|---|---|
| `OV_DOM_FIELDS` | render → browser | the detected DOM fields |
| `OV_REQ_DECRYPT` | browser → render | an armored PGP message |
| `OV_MUTATE_DOM_NODE` | browser → render | a replacement for a node |
| `OV_MUTATION_RESULT` | render → browser | the outcome of a mutation |
| `OV_READ_VALUE` | browser → render | the value of a node is needed again |
| `OV_DECRYPT_RESULT` | browser → render | the decrypted plaintext |
| `OV_REQ_FIELD_VALUE` | browser → render | the value of a node is needed |
| `OV_FIELD_VALUE` | render → browser | the value of a node |
| `OV_NAVIGATION` | browser → native | the current address |

The native UI is called back through JNI (`dev.onionvault.OvBridge`) and calls
into the engine through `dev.onionvault.OvNative`.

The `#[no_mangle]` JNI symbols live in the `cdylib` of the application
(`src/lib.rs`); they forward to the implementation in `ov-core`.  The shared
library of the APK is the only object that the Android runtime loads, so the
entry points have to be defined there.

## Building

### Requirements

* Rust 1.82 or newer with the `aarch64-linux-android` target,
* the Android NDK (r27c is used in development) and `cargo-ndk`,
* JDK 17 and the Android SDK (platform 35, build-tools 35.0.0),
* **the CEF Android distribution** for `arm64-v8a`.

### The CEF runtime

`cef-rs` does not ship Android bindings and its build script downloads a
*desktop* CEF distribution. `third_party/cef` and `third_party/cef-dll-sys`
are patched copies that

* alias the `aarch64-linux-gnu` bindings to `aarch64-linux-android`, and
* build against a CEF distribution that is provided through
  `CEF_ANDROID_ROOT` (or `CEF_PATH`).

CEF itself publishes binary distributions for Windows, Linux and macOS only;
there is no official Android build.  The engine for OnionVault comes from
[Cefrium](https://codeberg.org/cefrium/cef-android), the port of CEF to
`aarch64-linux-android`; its release 0.19.0 is built on Chromium
156.0.8078.80, the same Chromium release that the CEF 156 bindings of this
project expect.  The engine is inside the SDK of the release:

```sh
# The SDK of the release: the shared object and the resource files.
curl -L -o cefrium-sdk.aar \
  https://codeberg.org/cefrium/cef-android/releases/download/v0.19.0/cefrium-sdk-0.19.0.aar
echo "efb3f50dede4485215560455e16836671a7ebae32a35559bcfd4456d6667978b  cefrium-sdk.aar" \
  | sha256sum --check --strict -
unzip -q cefrium-sdk.aar 'jni/arm64-v8a/libcef.so' 'assets/*' -d cefrium

export CEF_ANDROID_ROOT=$PWD/cef-root
mkdir -p "$CEF_ANDROID_ROOT/lib" "$CEF_ANDROID_ROOT/locales"
cp cefrium/jni/arm64-v8a/libcef.so "$CEF_ANDROID_ROOT/lib/"
for file in cefrium/assets/*.pak* cefrium/assets/icudtl.dat*; do
    cp "$file" "$CEF_ANDROID_ROOT/$(basename "${file%+com.cefrium+}")"
done
cp cefrium/assets/stored-locales/en-US.pak* "$CEF_ANDROID_ROOT/locales/en-US.pak"
```

The checksum is the one that the release publishes in `SHA256SUMS-0.19.0`;
the continuous integration verifies it before it links against the engine.

The engine of Cefrium is free software under the **LGPL-3.0-or-later**, and
the upstream Chromium Embedded Framework under the BSD-3-Clause.  The engine
is a shared library: it is loaded at run time and can be replaced by the
user of the application.  The sources of Cefrium are published at
<https://codeberg.org/cefrium/cef-android> and the notices of the engine
travel with the release
(`cefrium-sdk-0.19.0-NOTICE.txt`); the third party dependencies of OnionVault
are listed in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

The licence of the engine has to be readable by the recipient of the binary.
The full text of every licence is therefore

  * in the [`licenses/`](licenses/) directory of this repository,
  * inside the APK, under `assets/licenses/`, where the application shows it
    (the button of the book in the top bar),
  * and attached to every release of the project, next to the APK.

The notice of the engine is 2.2 MiB and belongs to the exact build of the
engine, so it is not kept in the repository: the continuous integration
downloads it from the release of Cefrium, verifies it against the published
`SHA256SUMS-0.19.0` and puts it into the APK next to the other texts.

The sysroot has to provide the shared object and the resource files; the C++
wrapper is optional, because a distribution that ships one self-contained
`libcef.so` (as Cefrium does) exports every C API function from it:

```
$CEF_ANDROID_ROOT/
├── lib/
│   └── libcef.so       the Chromium runtime (packaged into the APK)
├── NOTICE.txt          the notice of the engine (packaged into the APK)
└── Resources/          resources.pak, chrome_100_percent.pak,
                        chrome_200_percent.pak, icudtl.dat, locales/…
```

`build.rs` copies the resource files into
`android/app/src/main/assets/cef/`, the Gradle build copies `libcef.so` into
the APK and the licence documents into
`android/app/src/main/assets/licenses/`.

The link step needs the NDK toolchain of API 29 or later: `libnativewindow`,
which carries the `ANativeWindow` of the off-screen rendering, was introduced
with API 24.  The Gradle build therefore calls

```sh
cargo ndk -t arm64-v8a --platform 29 -o app/src/main/jniLibs build --release
```

### Building the native library and the APK

```sh
export ANDROID_NDK_HOME=/opt/android-ndk-r27c
cargo install cargo-ndk

cd android
./gradlew assembleRelease
```

The Gradle build calls `cargo ndk -t arm64-v8a -o app/src/main/jniLibs build
--release` for the native part.

### Checking the sources without CEF

The build script of `cef-dll-sys` understands `ONIONVAULT_CEF_CHECK_ONLY=1`:
it then skips the C++ wrapper and only makes the bindings available, which
allows a full type check of the workspace without a CEF distribution.

```sh
export ONIONVAULT_CEF_CHECK_ONLY=1
cargo check --workspace
cargo test --workspace
```

## The Android side

| file | purpose |
|---|---|
| `MainActivity.kt` | the Compose activity: the surface, the CEF message pump (a `Choreographer` callback), the biometric prompt |
| `ui/BrowserScreen.kt` | the screen, the predictive back gesture, the PGP chips and the Tor circuit display |
| `ui/BrowserUiState.kt` | the immutable state of the screen |
| `ui/OnionBackStackManager.kt` | the snapshot back stack (12 entries) and the parsing of the field positions |
| `ui/previews/OnionVaultPreviews.kt` | the four previews of the screen |
| `OvSurfaceView.kt` | the `SurfaceView` that shows the off-screen rendered page and forwards the touch, key and IME events |
| `CefService.kt` | the CEF subprocesses (renderer, GPU, utility) — Android starts them as services |
| `CefResources.kt` | unpacks the CEF resource files from the assets |
| `OvNative.kt` | the JNI declarations of the engine |
| `OvBridge.kt` | the callbacks of the engine |

The CEF message loop is driven by the application: `external_message_pump` is
enabled and `MainActivity` calls `cef_do_message_loop_work()` once per frame.

## The CEF subprocesses

Android does not allow an application to fork a helper process, so the CEF
subprocesses (renderer, GPU and utility process) are Android services:
`CefRenderService`, `CefGpuService` and `CefUtilityService` all derive from
`CefService` and run `cef_execute_process()` with the command line that CEF
prepared.  CEF's Android port starts them through the
`--service-class=dev.onionvault.CefService` switch, which the command line of
the browser process carries.

## Tests

```sh
cargo test --workspace
```

* `ov-tor`: the `.onion` apex isolation, the SOCKS5 handshake and the strict
  mode of the local proxy,
* `ov-pgp`: generate → export → encrypt → decrypt → verify (including the
  check that the database never contains an armored private key),
* `ov-core`: the BGRA/RGBA swizzle of the frame copier and the translation of
  the Android key codes.

The workspace is also checked for the Android target:

```sh
export ANDROID_NDK_HOME=/opt/android-ndk-r27c
export CC_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang
export CXX_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang++
export AR_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
cargo check --workspace --target aarch64-linux-android
```

`cargo ndk` sets these variables itself, so the plain `cargo ndk` invocation
of the Gradle build needs no further configuration.

The previews are rendered on the JVM:

```sh
cd android
gradle :app:testDebugUnitTest --tests 'dev.onionvault.OnionVaultPreviewsTest'
```

## Continuous integration

`.github/workflows/ci-release.yml` runs on every push and has three jobs:

1. **Rust checks and tests** — the formatting, Clippy, the type check of the
   workspace, the unit tests and the integration tests (the PGP round trip and
   the Tor proxy), and the headless DOM fixture test of `ov-cef`,
2. **Android debug build** — the download of the CEF runtime for Android, the
   cross compilation for `aarch64-linux-android`, the Compose preview test,
   the debug APK and a check that the APK really carries the engine,
3. **Release** — a GitHub release with the APK and its SHA-256 sum.

The tags of the releases are unique (`build-<run number>-<commit>`), so that a
release never overwrites the artefact of an earlier build.

The pipeline publishes the **real** APK: the second job downloads the CEF
runtime (see [The CEF runtime](#the-cef-runtime)), links the native library of
OnionVault against it and fails the build if the engine, the native library or
the resource files are missing from the APK.

The workflow needs a repository on GitHub: the project is a git repository
without a remote, so nothing is published until it is pushed to one.

`.github/workflows/live-tor.yml` runs every night the two tests of the suite
that need the live Tor network: the bootstrap of the client and the tunnel to
a real onion service. They are marked with `#[ignore]`, so the hermetic suite
of every push stays offline and fast.

## Verification status

The sources were verified in the following way:

* `cargo fmt --all` — clean,
* `cargo check --workspace` — clean for the host,
* `cargo check --workspace --target aarch64-linux-android` — clean with the
  NDK r27c toolchain (`ONIONVAULT_CEF_CHECK_ONLY=1`),
* `cargo test --workspace` — 47 tests pass and one test is skipped (the Tor
  isolation and the SOCKS5 listener, the master key wrapping, the PGP keyring,
  the PGP round trip, the frame copier, the key translation, the DOM model and
  the headless DOM fixture test),
* `gradle :app:compileDebugKotlin` — clean,
* `gradle :app:testDebugUnitTest --tests 'dev.onionvault.OnionVaultPreviewsTest'`
  — the four previews render on the JVM,
* `cargo test -p ov-tor -- --ignored` — the embedded Arti client bootstraps
  against the live Tor network and exposes its SOCKS5 listener, and the
  client publishes a real onion service and reads the compose page of that
  service back through a real circuit,
* `cargo ndk -t arm64-v8a --platform 29 build --release` — the native library
  is linked against the real `libcef.so` of the CEF Android distribution,
* `gradle assembleDebug` — produces an installable APK (`dev.onionvault`,
  `minSdk 29`, `targetSdk 35`, `arm64-v8a`) that carries the engine
  (`lib/arm64-v8a/libcef.so`), the native library of OnionVault
  (`lib/arm64-v8a/libonion_vault.so`) and the resource files of Chromium
  (`assets/cef/`).

The link of the native library and the packaging of the CEF runtime need a
CEF Android distribution; `ONIONVAULT_CEF_CHECK_ONLY=1` switches exactly
those two steps off, which is why the native code can be checked against the
real CEF headers and bindings without the engine.

## Licence

Apache-2.0 OR MIT — the texts are in [`LICENSE-APACHE`](LICENSE-APACHE) and
[`LICENSE-MIT`](LICENSE-MIT). The dependencies keep their own licences, which
are listed in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
