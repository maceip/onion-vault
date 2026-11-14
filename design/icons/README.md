# The Trap icon lockups

The launcher icon of the application is generated from a single piece of
artwork, so that the icon, the adaptive icon, the themed icon, the store icon
and the wordmark lockup can never drift apart.

    python3 design/icons/trap_icon.py           # write the files
    python3 design/icons/trap_icon.py --check   # fail when they drifted

The artwork in `src/trap-onion.png` is the source: a smiling onion on a
teal badge.  The script cuts the onion out of the badge, and composes it on
the palette of the brand for every lockup.  It writes into
`android/app/src/main/res` and into `design/icons/out`.

The rendering is byte for byte reproducible, so `--check` compares the files of
the tree with a fresh rendering; the continuous integration runs it on every
push and fails the build when a lockup was edited by hand.

The script needs Pillow and NumPy, nothing else: no browser, no network.

## The lockups

| lockup | file | use |
| --- | --- | --- |
| hero icon | `mipmap-*/ic_launcher.png` | the launcher icon of Android 10 and later, and of every launcher that does not support adaptive icons |
| round icon | `mipmap-*/ic_launcher_round.png` | the icon of the launchers that ask for a round icon |
| adaptive foreground | `mipmap-*/ic_launcher_foreground.png` | the artwork of the adaptive icon (108dp canvas, the artwork inside the 72dp safe zone) |
| adaptive background | `drawable/ic_launcher_background.xml` | the gradient of the badge of the artwork, as a vector |
| themed icon | `mipmap-*/ic_launcher_monochrome.png` | the silhouette that Android 13 tints with the colours of the wallpaper |
| the mark | `drawable-nodpi/ov_trap_mark.png` | the onion alone, for the brand bar of the application |
| store icon | `design/icons/out/play-store-1024.png` | the icon of the app store listing (and `play-store-512.png`) |
| wordmark | `design/icons/out/trap-lockup.svg` | the icon next to the wordmark, for the about screen and for the documentation |

## How the onion is cut out

The badge of the artwork is a smooth gradient from teal to cyan.  The script
models that gradient with a quadratic per colour channel, fitted with robust
iterations that trim the onion away, and everything that the model cannot
explain is the onion.  The holes of the mark (the eyes, the mouth) are filled
afterwards, so the silhouette stays solid.

The mark is then placed on the gradient of the brand: teal `#1DE5C2` at the top
left, cyan `#16C2CF` in the middle and deep blue `#10A1D8` at the bottom
right, the same three stops as the adaptive icon.

The wordmark is set in the subset of DejaVu Sans Bold that is part of the
repository (`fonts/Trap-Bold.ttf`); it carries the glyphs of the wordmark
only, so that the lockup is the same on every machine and the check cannot
drift.  The licence of the font is next to it
(`fonts/LICENSE-DejaVu.txt`).  The vector lockup embeds the font, so it
renders the same everywhere; outline the text before it is used for anything
that has to look exactly the same everywhere.
