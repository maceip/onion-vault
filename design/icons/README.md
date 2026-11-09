# The OnionVault icon lockups

The launcher icon of the application is generated from a single source, so
that the icon, the adaptive icon, the themed icon and the wordmark lockup
can never drift apart.

    python3 design/icons/onionvault_icon.py

The script renders every lockup with the headless browser (Playwright) and
writes them into `android/app/src/main/res`.  It needs the Playwright
browser: `python3 -m playwright install chromium`.

## The lockups

| lockup | file | use |
| --- | --- | --- |
| hero icon | `mipmap-*/ic_launcher.png` | the launcher icon of Android 10 and later, and of every launcher that does not support adaptive icons |
| round icon | `mipmap-*/ic_launcher_round.png` | the icon of the launchers that ask for a round icon |
| adaptive foreground | `mipmap-*/ic_launcher_foreground.png` | the artwork of the adaptive icon (108dp canvas, the artwork inside the 72dp safe zone) |
| adaptive background | `drawable/ic_launcher_background.xml` | the violet field of the brand, as a vector |
| themed icon | `drawable/ic_launcher_monochrome.xml` | the silhouette that Android 13 tints with the colours of the wallpaper |
| store icon | `design/icons/out/play-store-512.png` | the icon of the app store listing |
| wordmark | `design/icons/out/onionvault-lockup.svg` | the icon next to the wordmark, for the about screen and for the documentation |

The design is a high gloss "liquid glass" browser window: a full browser
chrome (a tab, the address bar with a padlock and the indicator of the
tunnel) with the OnionVault mark, an onion with a key hole, in the page
area.  The artwork deliberately avoids SVG filters, so the files render
identically in Chromium, in cairosvg and on the device.

The wordmark is set in a system sans; outline the text before it is used
for anything that has to look exactly the same everywhere.
