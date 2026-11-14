# The Trap icon lockups

The launcher icon of the application is generated from a single bundle of
artwork, so that the icon, the adaptive icon, the themed icon, the store icon
and the wordmark lockup can never drift apart.

    python3 design/icons/trap_icon.py           # write the files
    python3 design/icons/trap_icon.py --check   # fail when they drifted

The bundle in `android/` is the source: the graffiti piece of the name, the
launcher icon of every density, the three layers of the adaptive icon and the
silhouette for the themed icons.  The script copies the bundle into the
resources of the application and composes the mark of the interface, the icon
of the listing and the wordmark out of the same pictures.  It writes into
`android/app/src/main/res` and into `design/icons/out`.

The rendering is byte for byte reproducible, so `--check` compares the files of
the tree with a fresh rendering; the continuous integration runs it on every
push and fails the build when a lockup was edited by hand.

The script needs Pillow, nothing else: no browser, no network.

## The lockups

| lockup | file | use |
| --- | --- | --- |
| hero icon | `mipmap-*/ic_launcher.png` | the launcher icon of Android 10 and later, and of every launcher that does not support adaptive icons |
| round icon | `mipmap-*/ic_launcher_round.png` | the icon of the launchers that ask for a round icon, cut out of the same artwork |
| adaptive foreground | `mipmap-xxxhdpi/ic_launcher_foreground.png` | the artwork of the adaptive icon (108dp canvas, the artwork inside the 72dp safe zone) |
| adaptive background | `mipmap-xxxhdpi/ic_launcher_background.png` | the black field behind the artwork |
| themed icon | `mipmap-xxxhdpi/ic_launcher_monochrome.png` | the silhouette that Android 13 tints with the colours of the wallpaper |
| the mark | `drawable-nodpi/ov_trap_mark.png` | the artwork in a rounded square, for the brand bar of the application |
| store icon | `design/icons/out/play-store-1024.png` | the icon of the app store listing (and `play-store-512.png`) |
| themed exports | `design/icons/out/ic_launcher_monochrome_*.png` | the silhouette in the sizes that the listing asks for |
| wordmark | `design/icons/out/trap-lockup.svg` | the icon next to the wordmark, for the about screen and for the documentation |

## The safe zone of the adaptive icon

The launcher masks the canvas of 108dp of an adaptive icon down to the central
72dp, and the shape of the mask is a circle on many devices.  The artwork of
the bundle fills the whole canvas, so it is scaled to the square that is
inscribed in that circle and centred: the piece stays whole, whatever the
mask of the launcher is.  The black field of the background layer fills the
rest of the icon.

The round variant of the legacy icon is cut out of the square picture of the
bundle with the same circle, because the bundle carries the square picture
for both of them.

The wordmark is set in the subset of DejaVu Sans Bold that is part of the
repository (`fonts/Trap-Bold.ttf`); it carries the glyphs of the wordmark
only, so that the lockup is the same on every machine and the check cannot
drift.  The licence of the font is next to it
(`fonts/LICENSE-DejaVu.txt`).  The vector lockup embeds the font, so it
renders the same everywhere; outline the text before it is used for anything
that has to look exactly the same everywhere.
