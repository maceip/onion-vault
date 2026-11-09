#!/usr/bin/env python3
"""Generate the OnionVault launcher icon lockups.

The icon is a single, parametric piece of artwork: a high gloss "liquid
glass" browser window -- a full browser chrome (tab strip, address bar with
a padlock) with the OnionVault mark (an onion with a key hole) in the page
area -- floating in the violet field of the brand.

Every lockup is rendered from the same source, so the icon, the adaptive
icon, the themed icon and the wordmark lockup can never drift apart:

    hero        the full icon on its squircle, for the app stores and for
                launchers that do not support adaptive icons
    foreground  the artwork of the adaptive icon (transparent, inside the
                72dp safe zone of the 108dp canvas)
    background  the violet field of the adaptive icon
    monochrome  the silhouette for the Android 13+ themed icons
    lockup      the wordmark next to the icon, for the about screen

The artwork deliberately avoids SVG filters: every soft edge is a gradient,
so the files render identically in Chromium, in cairosvg and on the device.

Usage:
    python3 design/icons/onionvault_icon.py [--out DIR]
"""

from __future__ import annotations

import argparse
import io
import json
from pathlib import Path

# The OnionVault mark: an onion with a key hole, in the coordinate system
# of the original launcher icon of the application (108x108).
ONION = (
    "M54,28 C48,28 44,30 42,34 C34,40 29,49 29,59 C29,73 40,84 54,84 "
    "C68,84 79,73 79,59 C79,49 74,40 66,34 C64,30 60,28 54,28 Z"
)
SPROUT = "M52.4,28.6 C49.6,21.6 52,13.4 58.4,9.6 C55.6,16 54.4,22.4 56.2,28.2 Z"
KEYHOLE_DISC = "M54,55 m-5.2,0 a5.2,5.2 0 1,0 10.4,0 a5.2,5.2 0 1,0 -10.4,0"
KEYHOLE_SLOT = "M51.4,59 h5.2 v13 h-5.2 z"

# The bounding box of the mark above.
ONION_BOX = (29.0, 9.6, 79.0, 84.0)

# The brand palette.  The violet is the colour of the original launcher
# icon, the field is the background colour of the application.
FIELD_TOP = "#241546"
FIELD_MID = "#150E28"
FIELD_BOTTOM = "#080A0F"
VIOLET = "#8B5CF6"
MAGENTA = "#C026D3"
TEAL = "#2DD4BF"
INK = "#3B0F73"

# The layout of the browser window for every lockup.  The adaptive icon
# has to survive a circular mask of 72dp, so its window is smaller and has
# a larger corner radius than the hero icon.
LAYOUTS = {
    "hero": {"x": 13.5, "y": 19.5, "w": 81.0, "h": 69.0, "rx": 21.0},
    "adaptive": {"x": 23.0, "y": 27.0, "w": 62.0, "h": 54.0, "rx": 18.0},
}


def mark_transform(center_x: float, center_y: float, height: float) -> str:
    """The transform that places the mark at `center` with the given height."""
    x0, y0, x1, y1 = ONION_BOX
    scale = height / (y1 - y0)
    tx = center_x - scale * (x0 + x1) / 2.0
    ty = center_y - scale * (y0 + y1) / 2.0
    return f"translate({tx:.3f} {ty:.3f}) scale({scale:.4f})"


def defs(layout: dict) -> str:
    """The gradients and the clip paths of the artwork."""
    x, y, w, h = layout["x"], layout["y"], layout["w"], layout["h"]
    return f"""
  <defs>
    <linearGradient id="field" x1="0" y1="0" x2="0.72" y2="1">
      <stop offset="0" stop-color="{FIELD_TOP}"/>
      <stop offset="0.45" stop-color="{FIELD_MID}"/>
      <stop offset="1" stop-color="{FIELD_BOTTOM}"/>
    </linearGradient>
    <radialGradient id="glowViolet" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="{VIOLET}" stop-opacity="0.62"/>
      <stop offset="0.55" stop-color="{VIOLET}" stop-opacity="0.18"/>
      <stop offset="1" stop-color="{VIOLET}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="glowMagenta" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="{MAGENTA}" stop-opacity="0.42"/>
      <stop offset="0.6" stop-color="{MAGENTA}" stop-opacity="0.10"/>
      <stop offset="1" stop-color="{MAGENTA}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="glowTeal" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="{TEAL}" stop-opacity="0.16"/>
      <stop offset="0.6" stop-color="{TEAL}" stop-opacity="0.04"/>
      <stop offset="1" stop-color="{TEAL}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="glass" x1="0" y1="0" x2="0.85" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.20"/>
      <stop offset="0.42" stop-color="#FFFFFF" stop-opacity="0.075"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.028"/>
    </linearGradient>
    <linearGradient id="rim" x1="0.1" y1="0" x2="0.9" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.95"/>
      <stop offset="0.28" stop-color="#FFFFFF" stop-opacity="0.36"/>
      <stop offset="0.62" stop-color="#FFFFFF" stop-opacity="0.12"/>
      <stop offset="1" stop-color="#C4B5FD" stop-opacity="0.58"/>
    </linearGradient>
    <linearGradient id="rimSoft" x1="0.1" y1="0" x2="0.9" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.55"/>
      <stop offset="0.55" stop-color="#FFFFFF" stop-opacity="0.12"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.26"/>
    </linearGradient>
    <radialGradient id="streak" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.34"/>
      <stop offset="0.55" stop-color="#FFFFFF" stop-opacity="0.10"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="glint" x1="0" y1="0" x2="1" y2="0.6">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0"/>
      <stop offset="0.35" stop-color="#FFFFFF" stop-opacity="0.34"/>
      <stop offset="0.52" stop-color="#FFFFFF" stop-opacity="0.08"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <radialGradient id="coolGlow" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#A78BFA" stop-opacity="0.26"/>
      <stop offset="1" stop-color="#A78BFA" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="innerShade" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#0B0E11" stop-opacity="0"/>
      <stop offset="0.55" stop-color="#0B0E11" stop-opacity="0.10"/>
      <stop offset="1" stop-color="#0B0E11" stop-opacity="0.42"/>
    </linearGradient>
    <linearGradient id="tabFill" x1="0" y1="0" x2="0.4" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.26"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.07"/>
    </linearGradient>
    <linearGradient id="barFill" x1="0" y1="0" x2="0.3" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.20"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.05"/>
    </linearGradient>
    <linearGradient id="markFill" x1="0.15" y1="0" x2="0.85" y2="1">
      <stop offset="0" stop-color="#FFFFFF"/>
      <stop offset="0.5" stop-color="#F3E8FF"/>
      <stop offset="1" stop-color="#C4B5FD"/>
    </linearGradient>
    <radialGradient id="markGlow" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="{VIOLET}" stop-opacity="0.55"/>
      <stop offset="1" stop-color="{VIOLET}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="caustic" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="{MAGENTA}" stop-opacity="0.40"/>
      <stop offset="1" stop-color="{MAGENTA}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="favicon" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#5EEAD4"/>
      <stop offset="1" stop-color="{VIOLET}"/>
    </linearGradient>
    <clipPath id="windowClip">
      <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{layout['rx']}"/>
    </clipPath>
    <clipPath id="markClip">
      <path d="{ONION}" transform="{mark_transform(x + w / 2, y + h - 20, h - 40)}"/>
      <path d="{SPROUT}" transform="{mark_transform(x + w / 2, y + h - 20, h - 40)}"/>
    </clipPath>
  </defs>"""


def field(include_squircle: bool) -> str:
    """The violet field with the aurora glows."""
    if include_squircle:
        shape = '<rect x="0" y="0" width="108" height="108" rx="24" fill="url(#field)"/>'
    else:
        shape = '<rect x="0" y="0" width="108" height="108" fill="url(#field)"/>'
    return f"""
  <g>
    {shape}
    <circle cx="26" cy="16" r="56" fill="url(#glowViolet)"/>
    <circle cx="94" cy="92" r="48" fill="url(#glowMagenta)"/>
    <circle cx="26" cy="104" r="34" fill="url(#glowTeal)"/>
  </g>"""


def browser_window(layout: dict) -> str:
    """The glass browser window with the full browser chrome."""
    x, y, w, h = layout["x"], layout["y"], layout["w"], layout["h"]
    rx = layout["rx"]

    # The tab strip: the active tab, its favicon, the address of the page
    # and the button that opens a new tab.
    tab_x, tab_y = x + 7.5, y + 3.5
    tab_w, tab_h = 34.0, 13.0
    favicon_x, favicon_y = tab_x + 6.0, tab_y + 6.5
    url_x, url_y = tab_x + 13.0, tab_y + 5.3
    url_w = 13.0
    close_x = tab_x + tab_w - 5.0

    # The address bar with the padlock and the "connected" indicator.
    bar_x, bar_y = x + 7.5, y + 21.0
    bar_w, bar_h = w - 15.0, 13.5
    lock_x, lock_y = bar_x + 5.5, bar_y + 4.0
    text_x = bar_x + 13.5
    text_w = bar_w - 25.0
    dot_x = bar_x + bar_w - 6.5

    return f"""
  <g>
    <!-- the caustic light under the window -->
    <ellipse cx="{x + w / 2:.2f}" cy="{y + h + 1.5:.2f}" rx="{w * 0.42:.2f}" ry="6.5"
             fill="url(#caustic)"/>

    <!-- the glass body of the window -->
    <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="url(#glass)"/>

    <!-- the specular light that makes the glass look wet -->
    <g clip-path="url(#windowClip)">
      <ellipse cx="{x + w * 0.24:.2f}" cy="{y + h * 0.06:.2f}" rx="{w * 0.46:.2f}" ry="{h * 0.30:.2f}"
               fill="url(#streak)" transform="rotate(-16 {x + w * 0.24:.2f} {y + h * 0.06:.2f})"/>
      <ellipse cx="{x + w * 0.88:.2f}" cy="{y + h * 0.96:.2f}" rx="{w * 0.36:.2f}" ry="{h * 0.26:.2f}"
               fill="url(#coolGlow)" transform="rotate(-16 {x + w * 0.88:.2f} {y + h * 0.96:.2f})"/>
      <!-- the hard glint of a wet surface -->
      <path d="M{x - w * 0.10:.2f} {y + h * 0.62:.2f} C{x + w * 0.16:.2f} {y + h * 0.18:.2f} {x + w * 0.34:.2f} {y + h * 0.02:.2f} {x + w * 0.66:.2f} {y - h * 0.06:.2f}
               L{x + w * 0.50:.2f} {y + h * 0.02:.2f} C{x + w * 0.26:.2f} {y + h * 0.14:.2f} {x + w * 0.10:.2f} {y + h * 0.44:.2f} {x - w * 0.10:.2f} {y + h * 0.80:.2f} Z"
            fill="url(#glint)"/>
      <rect x="{x}" y="{y + h * 0.52:.2f}" width="{w}" height="{h * 0.48:.2f}" fill="url(#innerShade)"/>
    </g>

    <!-- the rim of the window: the bright edge of the glass -->
    <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="none"
          stroke="url(#rim)" stroke-width="1.8"/>

    <!-- the browser chrome -->
    <g>
      <!-- the active tab -->
      <rect x="{tab_x}" y="{tab_y}" width="{tab_w}" height="{tab_h}" rx="6.5"
            fill="url(#tabFill)" stroke="url(#rimSoft)" stroke-width="0.7"/>
      <circle cx="{favicon_x:.2f}" cy="{favicon_y:.2f}" r="2.4" fill="url(#favicon)"/>
      <rect x="{url_x:.2f}" y="{url_y:.2f}" width="{url_w}" height="2.0" rx="1.0"
            fill="#FFFFFF" fill-opacity="0.34"/>
      <path d="M{close_x - 1.6:.2f} {tab_y + 4.6:.2f} L{close_x + 1.6:.2f} {tab_y + 7.8:.2f}
               M{close_x + 1.6:.2f} {tab_y + 4.6:.2f} L{close_x - 1.6:.2f} {tab_y + 7.8:.2f}"
            stroke="#FFFFFF" stroke-opacity="0.30" stroke-width="0.85" stroke-linecap="round"/>

      <!-- the button that opens a new tab -->
      <path d="M{x + w - 15.5:.2f} {tab_y + 6.5:.2f} L{x + w - 10.5:.2f} {tab_y + 6.5:.2f}
               M{x + w - 13.0:.2f} {tab_y + 4.0:.2f} L{x + w - 13.0:.2f} {tab_y + 9.0:.2f}"
            stroke="#FFFFFF" stroke-opacity="0.42" stroke-width="1.1" stroke-linecap="round"/>

      <!-- the address bar -->
      <rect x="{bar_x}" y="{bar_y}" width="{bar_w}" height="{bar_h}" rx="6.75"
            fill="url(#barFill)" stroke="url(#rimSoft)" stroke-width="0.7"/>

      <!-- the padlock: the traffic never leaves the tunnel -->
      <rect x="{lock_x:.2f}" y="{lock_y + 2.6:.2f}" width="6.2" height="5.4" rx="1.5"
            fill="#FFFFFF" fill-opacity="0.94"/>
      <path d="M{lock_x + 1.3:.2f} {lock_y + 2.6:.2f} v-1.5 a1.8 1.8 0 0 1 3.6 0 v1.5"
            fill="none" stroke="#FFFFFF" stroke-opacity="0.94" stroke-width="1.25" stroke-linecap="round"/>

      <!-- the address of the hidden service -->
      <rect x="{text_x:.2f}" y="{bar_y + 5.6:.2f}" width="{text_w:.2f}" height="2.3" rx="1.15"
            fill="#FFFFFF" fill-opacity="0.32"/>

      <!-- the indicator that the tunnel is up -->
      <circle cx="{dot_x:.2f}" cy="{bar_y + bar_h / 2:.2f}" r="4.0" fill="{TEAL}" fill-opacity="0.16"/>
      <circle cx="{dot_x:.2f}" cy="{bar_y + bar_h / 2:.2f}" r="2.1" fill="{TEAL}" fill-opacity="0.92"/>
    </g>
  </g>"""


def mark(layout: dict) -> str:
    """The OnionVault mark: the onion with the key hole."""
    x, y, w, h = layout["x"], layout["y"], layout["w"], layout["h"]
    cx, cy = x + w / 2, y + h - 20
    height = h - 40
    transform = mark_transform(cx, cy, height)
    return f"""
  <g>
    <!-- the glow of the mark -->
    <circle cx="{cx:.2f}" cy="{cy:.2f}" r="{height * 0.85:.2f}" fill="url(#markGlow)"/>
    <g transform="{transform}">
      <path d="{SPROUT}" fill="url(#markFill)"/>
      <path d="{ONION}" fill="url(#markFill)"/>
      <!-- the specular highlight of the glass -->
      <g clip-path="url(#markClip)">
        <ellipse cx="44" cy="38" rx="15" ry="8" fill="#FFFFFF" fill-opacity="0.5"
                 transform="rotate(-26 44 38)"/>
      </g>
      <path d="{KEYHOLE_DISC}" fill="{INK}"/>
      <path d="{KEYHOLE_SLOT}" fill="{INK}"/>
    </g>
  </g>"""


def svg(variant: str, size: int = 108) -> str:
    """The SVG source of one variant."""
    if variant == "background":
        body = field(include_squircle=False)
        return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 108 108">
{defs(LAYOUTS['adaptive'])}
{body}
</svg>"""

    if variant == "monochrome":
        layout = LAYOUTS["adaptive"]
        x, y, w, h = layout["x"], layout["y"], layout["w"], layout["h"]
        cx, cy = x + w / 2, y + h - 20
        transform = mark_transform(cx, cy, h - 40)
        return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 108 108">
  <defs>
    <clipPath id="windowClip">
      <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{layout['rx']}"/>
    </clipPath>
  </defs>
  <g fill="#000000">
    <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{layout['rx']}" fill="none"
          stroke="#000000" stroke-width="2.6"/>
    <rect x="{x + 7.5}" y="{y + 21.0}" width="{w - 15.0}" height="13.5" rx="6.75"/>
    <circle cx="{x + w / 2:.2f}" cy="{cy:.2f}" r="{(h - 40) * 0.42:.2f}"/>
  </g>
  <g transform="{transform}">
    <path d="{ONION}"/>
  </g>
</svg>"""

    layout = LAYOUTS["hero" if variant == "hero" else "adaptive"]
    parts = []
    if variant == "hero":
        parts.append(field(include_squircle=True))
    parts.append(browser_window(layout))
    parts.append(mark(layout))
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 108 108">
{defs(layout)}
{"".join(parts)}
</svg>"""


def vector_background() -> str:
    """The background layer of the adaptive icon, as an Android vector."""
    return """<?xml version="1.0" encoding="utf-8"?>
<!-- The violet field of the adaptive icon. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path android:pathData="M0,0h108v108h-108z">
        <aapt:attr name="android:fillColor">
            <gradient
                android:type="linear"
                android:startX="0"
                android:startY="0"
                android:endX="77"
                android:endY="108">
                <item android:offset="0" android:color="#FF241546" />
                <item android:offset="0.45" android:color="#FF150E28" />
                <item android:offset="1" android:color="#FF080A0F" />
            </gradient>
        </aapt:attr>
    </path>
    <path android:pathData="M26,16m-56,0a56,56 0,1 1,112 0a56,56 0,1 1,-112 0">
        <aapt:attr name="android:fillColor">
            <gradient
                android:type="radial"
                android:centerX="26"
                android:centerY="16"
                android:gradientRadius="56">
                <item android:offset="0" android:color="#9E8B5CF6" />
                <item android:offset="0.55" android:color="#2E8B5CF6" />
                <item android:offset="1" android:color="#008B5CF6" />
            </gradient>
        </aapt:attr>
    </path>
    <path android:pathData="M94,92m-48,0a48,48 0,1 1,96 0a48,48 0,1 1,-96 0">
        <aapt:attr name="android:fillColor">
            <gradient
                android:type="radial"
                android:centerX="94"
                android:centerY="92"
                android:gradientRadius="48">
                <item android:offset="0" android:color="#6BC026D3" />
                <item android:offset="0.6" android:color="#1AC026D3" />
                <item android:offset="1" android:color="#00C026D3" />
            </gradient>
        </aapt:attr>
    </path>
    <path android:pathData="M26,104m-34,0a34,34 0,1 1,68 0a34,34 0,1 1,-68 0">
        <aapt:attr name="android:fillColor">
            <gradient
                android:type="radial"
                android:centerX="26"
                android:centerY="104"
                android:gradientRadius="34">
                <item android:offset="0" android:color="#292DD4BF" />
                <item android:offset="0.6" android:color="#0A2DD4BF" />
                <item android:offset="1" android:color="#002DD4BF" />
            </gradient>
        </aapt:attr>
    </path>
</vector>
"""


def vector_monochrome() -> str:
    """The themed icon layer (Android 13+), as an Android vector."""
    layout = LAYOUTS["adaptive"]
    x, y, w, h = layout["x"], layout["y"], layout["w"], layout["h"]
    cx, cy = x + w / 2, y + h - 20
    transform = mark_transform(cx, cy, h - 40)
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!--
    The silhouette of the icon for the themed icons of Android 13 and later:
    the platform tints this layer with the colours of the wallpaper.
-->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:pathData="M{x},{y + layout['rx']} a{layout['rx']},{layout['rx']} 0 0 1 {layout['rx']},{-layout['rx']} h{w - 2 * layout['rx']} a{layout['rx']},{layout['rx']} 0 0 1 {layout['rx']},{layout['rx']} v{h - 2 * layout['rx']} a{layout['rx']},{layout['rx']} 0 0 1 {-layout['rx']},{layout['rx']} h{-(w - 2 * layout['rx'])} a{layout['rx']},{layout['rx']} 0 0 1 {-layout['rx']},{-layout['rx']} z"
        android:strokeColor="#FF000000"
        android:strokeWidth="2.6"
        android:fillColor="#00000000" />
    <path
        android:pathData="M{x + 7.5},{y + 27.75} h{w - 15} a6.75,6.75 0 0 1 0,13.5 h{-(w - 15)} a6.75,6.75 0 0 1 0,-13.5 z"
        android:fillColor="#FF000000" />
    <path
        android:pathData="M{cx:.2f},{cy:.2f} m-{(h - 40) * 0.42:.2f},0 a{(h - 40) * 0.42:.2f},{(h - 40) * 0.42:.2f} 0 1 1,{(h - 40) * 0.84:.2f} 0 a{(h - 40) * 0.42:.2f},{(h - 40) * 0.42:.2f} 0 1 1,-{(h - 40) * 0.84:.2f} 0"
        android:fillColor="#FF000000" />
    <group
        android:translateX="{x + w / 2 - 54 * (h - 40) / 74:.3f}"
        android:translateY="{cy - 47 * (h - 40) / 74:.3f}"
        android:scaleX="{(h - 40) / 74:.4f}"
        android:scaleY="{(h - 40) / 74:.4f}">
        <path android:pathData="{ONION}" android:fillColor="#FF000000" />
        <path android:pathData="{SPROUT}" android:fillColor="#FF000000" />
    </group>
</vector>
"""


def adaptive_icon_xml() -> str:
    """The adaptive icon of the launcher."""
    return """<?xml version="1.0" encoding="utf-8"?>
<!--
    The adaptive icon of OnionVault (Android 8 and later).  The artwork is
    the glass browser window, the background is the violet field of the
    brand and the monochrome layer is used for the themed icons.
-->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
    <monochrome android:drawable="@drawable/ic_launcher_monochrome" />
</adaptive-icon>
"""


def svg_lockup() -> str:
    """The horizontal lockup: the icon next to the wordmark."""
    layout = LAYOUTS["hero"]
    icon = svg("hero").replace(
        '<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108" viewBox="0 0 108 108">',
        '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 108 108" '
        'x="0" y="4">',
    ).replace("</svg>", "")
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="560" height="108" viewBox="0 0 560 108">
  <defs>
    <linearGradient id="wordmark" x1="0" y1="18" x2="0" y2="70" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#FFFFFF"/>
      <stop offset="1" stop-color="#DDD6FE"/>
    </linearGradient>
  </defs>
  <g>
  {icon}
  </g>
  <text x="118" y="62" font-family="Liberation Sans, Roboto, Helvetica, Arial, sans-serif"
        font-size="44" font-weight="bold" letter-spacing="-0.6" fill="url(#wordmark)">OnionVault</text>
  <text x="121" y="84" font-family="Liberation Sans, Roboto, Helvetica, Arial, sans-serif"
        font-size="12.5" letter-spacing="4.2" fill="#A78BFA" fill-opacity="0.92">TOR BROWSER</text>
</svg>"""


# The lockups that are written into the Android resource tree.  The name is
# the file name without the extension, the densities are the launcher icon
# sizes of the platform.
DENSITIES = {
    "mdpi": 1.0,
    "hdpi": 1.5,
    "xhdpi": 2.0,
    "xxhdpi": 3.0,
    "xxxhdpi": 4.0,
}
LEGACY_SIZE = 48     # launcher icon of the legacy (pre API 26) launchers
ADAPTIVE_SIZE = 108  # canvas of the adaptive icon


def render(svg_source: str, pixels: int) -> bytes:
    """Rasterise an SVG with the headless browser."""
    from playwright.sync_api import sync_playwright

    html = (
        "<!DOCTYPE html><html><head><meta charset='utf-8'><style>"
        "html,body{margin:0;padding:0;background:transparent}"
        f"svg{{display:block;width:{pixels}px;height:{pixels}px}}"
        "</style></head><body>" + svg_source + "</body></html>"
    )
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": pixels, "height": pixels},
                                device_scale_factor=1)
        page.set_content(html)
        png = page.screenshot(omit_background=True, type="png")
        browser.close()
    return png


def render_lockup(svg_source: str, width: int, height: int) -> bytes:
    """Rasterise the wordmark lockup."""
    from playwright.sync_api import sync_playwright

    html = (
        "<!DOCTYPE html><html><head><meta charset='utf-8'><style>"
        "html,body{margin:0;padding:0;background:transparent}"
        f"svg{{display:block;width:{width}px;height:{height}px}}"
        "</style></head><body>" + svg_source + "</body></html>"
    )
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": width, "height": height},
                                device_scale_factor=1)
        page.set_content(html)
        png = page.screenshot(omit_background=True, type="png")
        browser.close()
    return png


def rounded(source: bytes, size: int, radius: int) -> bytes:
    """Round the corners of a PNG (the round launcher icon)."""
    from PIL import Image, ImageDraw

    image = Image.open(io.BytesIO(source)).convert("RGBA")
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)
    image.putalpha(mask)
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return buffer.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=None,
                        help="the directory of the generated lockups")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[2]
    res = root / "android/app/src/main/res"
    out = Path(args.out) if args.out else root / "design/icons/out"
    out.mkdir(parents=True, exist_ok=True)

    written: dict[str, str] = {}

    # The adaptive icon: the artwork and the background of the launcher.
    for density, factor in DENSITIES.items():
        folder = res / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        pixels = int(ADAPTIVE_SIZE * factor)
        (folder / "ic_launcher_foreground.png").write_bytes(
            render(svg("foreground"), pixels)
        )
        written[f"mipmap-{density}/ic_launcher_foreground.png"] = f"{pixels}x{pixels}"

    # The legacy launcher icons.
    for density, factor in DENSITIES.items():
        folder = res / f"mipmap-{density}"
        pixels = int(LEGACY_SIZE * factor)
        square = render(svg("hero"), pixels)
        (folder / "ic_launcher.png").write_bytes(square)
        (folder / "ic_launcher_round.png").write_bytes(rounded(square, pixels, 0))
        written[f"mipmap-{density}/ic_launcher.png"] = f"{pixels}x{pixels}"
        written[f"mipmap-{density}/ic_launcher_round.png"] = f"{pixels}x{pixels}"

    # The vector layers of the adaptive icon.
    drawable = res / "drawable"
    (drawable / "ic_launcher_background.xml").write_text(vector_background())
    (drawable / "ic_launcher_monochrome.xml").write_text(vector_monochrome())

    # The adaptive icon itself.
    anydpi = res / "mipmap-anydpi-v26"
    anydpi.mkdir(parents=True, exist_ok=True)
    (anydpi / "ic_launcher.xml").write_text(adaptive_icon_xml())
    (anydpi / "ic_launcher_round.xml").write_text(adaptive_icon_xml())

    # The store listing and the brand assets.
    (out / "play-store-512.png").write_bytes(render(svg("hero"), 512))
    (out / "play-store-1024.png").write_bytes(render(svg("hero"), 1024))
    (out / "onionvault-lockup.svg").write_text(svg_lockup())
    (out / "onionvault-lockup.png").write_bytes(render_lockup(svg_lockup(), 1040, 216))

    # The sources, so that every lockup can be regenerated.
    for variant in ("hero", "foreground", "background", "monochrome"):
        (out / f"ic_launcher_{variant}.svg").write_text(svg(variant))

    print(json.dumps(written, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
