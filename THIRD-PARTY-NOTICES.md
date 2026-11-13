# Third party notices

OnionVault links against the following third party software. The licences
apply to those parts, not to the code of OnionVault itself, which is dual
licensed under Apache-2.0 OR MIT.

## Vendored in this repository

| component | origin | licence |
|---|---|---|
| `third_party/cef` | [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs) 156.4.0+156.0.26, patched for `aarch64-linux-android` | Apache-2.0 OR MIT |
| `third_party/cef-dll-sys` | [tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs) 156.4.0+156.0.26, patched for `aarch64-linux-android` | Apache-2.0 OR MIT |

The patches of the vendored crates are limited to the build script (the
Android API level, the location of the CEF distribution) and to the aliases
of the Android target. The original licence headers of the sources are
untouched.

## Used at runtime

| component | origin | licence |
|---|---|---|
| Chromium Embedded Framework | [Chromium Embedded Framework](https://github.com/chromiumembedded/cef) 156.0.26, via the Android port [cefrium/cef-android](https://codeberg.org/cefrium/cef-android) 0.19.0 (Chromium 156.0.8078.80) | LGPL-3.0-or-later (the port), BSD-3-Clause (the upstream CEF and Chromium) |
| sequoia-openpgp | [sequoia-pgp/sequoia-openpgp](https://gitlab.com/sequoia-pgp/sequoia-openpgp) 1.22 | LGPL-2.0-or-later |
| arti-client, tor-rtcompat | [artifacts.gitlab.torproject.org/tpo/core/arti](https://gitlab.torproject.org/tpo/core/arti) 0.47 | MIT OR Apache-2.0 |
| rustls | [rustls/rustls](https://github.com/rustls/rustls) | Apache-2.0 OR ISC OR MIT |
| rusqlite | [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | MIT |
| ndk, jni | [rust-android/ndk](https://github.com/rust-android/ndk), [rust-jni/jni](https://github.com/rust-jni/jni) | MIT OR Apache-2.0 |

The CEF engine is **not** redistributed in this repository: the continuous
integration downloads it, links against it and packages it into the APK. The
sources of `sequoia-openpgp` and of the other dependencies are published by
their authors under the licences above; the complete corresponding source of
the LGPL component is available from its repository and from the crates.io
release that the lock file pins.

`sequoia-openpgp` is LGPL-2.0-or-later and is linked into the native library
of the application.  The complete corresponding source of it is the content
of this repository together with the crates that the lock file pins, so that
the native library can be rebuilt and relinked; the build script of the
application (`build.rs` and the crates of `crates/`) is part of it.

The engine of Cefrium (LGPL-3.0-or-later) is loaded as a shared library at
run time and can be replaced by the user of the application.  Its sources
are published at <https://codeberg.org/cefrium/cef-android>, its notices
travel with the release as `cefrium-sdk-0.19.0-NOTICE.txt`, and the
continuous integration verifies the checksum of the download before it links
against it.

## The licence texts

The full text of every licence that applies to a component is in the
[`licenses/`](licenses/) directory of this repository:

| file | component | licence |
|---|---|---|
| `LGPL-3.0.txt` | the CEF engine of Cefrium | LGPL-3.0-or-later |
| `GPL-3.0.txt` | the GNU GPL that the LGPL-3.0 incorporates | GPL-3.0-or-later |
| `LGPL-2.1.txt` | sequoia-openpgp | LGPL-2.0-or-later |

The texts are inside the APK as well, under `assets/licenses/`, where the
application shows them, and they are attached to every GitHub Release next to
the APK.  The notice of the engine, which carries the licences of the
Chromium sources and of the libraries they are built from, is downloaded by
the continuous integration together with the engine and is put into the APK
next to these texts.

The artwork of the launcher icon and of the interface was made for OnionVault
and is covered by the licence of this repository.
