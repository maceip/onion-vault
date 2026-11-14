# The licences of the third party components

The full text of every licence that applies to a component of PlugTalk lives
in this directory.  The files are shipped inside the APK, so that a user of the
application can read them without leaving it, and they are attached to every
release of the project.

| file | component | licence |
|---|---|---|
| `LGPL-3.0.txt` | the CEF engine of [Cefrium](https://codeberg.org/cefrium/cef-android) | LGPL-3.0-or-later |
| `GPL-3.0.txt` | the GNU GPL that the LGPL-3.0 incorporates | GPL-3.0-or-later |
| `LGPL-2.1.txt` | [sequoia-openpgp](https://gitlab.com/sequoia-pgp/sequoia-openpgp) | LGPL-2.0-or-later |

The LGPL-3.0 is the licence of the engine: it incorporates the terms and
conditions of the GPL-3.0 and adds the permissions of the LGPL.  Both texts are
therefore needed; `LGPL-3.0.txt` carries the additional permissions and the
GPL-3.0 in one file, `GPL-3.0.txt` is the GPL on its own.

The texts are the ones of the Free Software Foundation, taken from the
[licence list of SPDX](https://github.com/spdx/license-list-data).  They are
verbatim copies; only the line endings are normalised.

## Where the documents are

The texts are shipped three times over, so that a recipient of the binary can
always read them:

  * here, in the repository, under their canonical names;
  * inside the APK, under `assets/licenses/`, where the application shows
    them: the button of the book in the upper bar opens the list;
  * next to the APK, as attachments of every GitHub Release.

The Gradle build copies the `*.txt` files of this directory into the assets
of the application; it copies the notice of the engine next to them, when
the engine is present.

## The notice of the engine

The engine carries a notice of its own, with the licences of the Chromium
sources and of every library they are built from.  That notice is 2.2 MiB, it
belongs to the exact build of the engine and it is therefore not kept in this
repository: the continuous integration downloads it from the release of
Cefrium, verifies it against the published SHA-256 of the release and puts it
into the APK next to these texts.

## Why the engine may be replaced

The engine is loaded as a shared library at run time, it is not linked into
the native library of the application.  A user of PlugTalk can therefore
replace `lib/arm64-v8a/libcef.so` with another build of the same engine, which
is what the LGPL-3.0 asks for: the work that uses the library has to be
distributed in a form that allows the library to be replaced.

The sources of the engine are published at
<https://codeberg.org/cefrium/cef-android>; the sources of the application
itself, together with the pinned versions of its Rust dependencies, are the
repository of this project.
