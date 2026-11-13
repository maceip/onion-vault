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
| Chromium Embedded Framework | [Chromium Embedded Framework](https://github.com/chromiumembedded/cef) 156.0.26, via the Android port [cefrium/cef-android](https://codeberg.org/cefrium/cef-android) 0.18.0 | BSD-3-Clause |
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

The artwork of the launcher icon and of the interface was made for OnionVault
and is covered by the licence of this repository.
