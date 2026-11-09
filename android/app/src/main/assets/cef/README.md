# The CEF resources

The Chromium Embedded Framework needs its resource files on the file system:

| file | purpose |
|---|---|---|
| `cef.pak` | the strings and the resources of CEF |
| `cef_100_percent.pak`, `cef_200_percent.pak` | the scaled resources |
| `icudtl.dat` | the ICU data (character sets, collation) |
| `locales/*.pak` | the translations |

They are **not** checked in: they are part of the CEF Android distribution
and are copied into this directory by the build script of the workspace
(`build.rs`), which reads `CEF_ANDROID_ROOT`.

At runtime `CefResources.unpack()` copies them into the private directory of
the application (`filesDir/cef`), because Chromium maps the files into its
memory and cannot read them from a compressed asset.
