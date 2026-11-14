#!/usr/bin/env python3
"""Generate every Trap icon out of the icon bundle of the artwork.

    python3 design/icons/trap_icon.py           # write the files
    python3 design/icons/trap_icon.py --check   # fail when they drifted

The bundle in `design/icons/android/` is the source: it carries the artwork
of the graffiti piece, the launcher icons of every density, the three layers
of the adaptive icon and the silhouette for the themed icons of Android 13.

The script copies the bundle into the resources of the application and
composes the few lockups of the documentation out of the same artwork, so
the launcher icon, the mark of the interface, the icon of the listing and
the wordmark can never drift apart.

The output is byte for byte reproducible, and `--check` compares the files
of the tree with a fresh rendering, which is what the pipeline of the
continuous integration runs.

Only Pillow is needed; it is in the image of the builder.
"""

from __future__ import annotations

import argparse
import base64
import io
import math
import sys
import tempfile
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError as exc:  # pragma: no cover - the dependency is documented
    raise SystemExit(f"trap_icon.py needs Pillow: {exc}")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
BUNDLE = HERE / "android"
OUT = HERE / "out"
RES = ROOT / "android" / "app" / "src" / "main" / "res"

# The densities of the launcher icons of the bundle.
DENSITIES = ("mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi")

# The canvas of an adaptive icon is 108dp and the launcher shows the central
# 72dp of it.  The artwork has to fit into the circle that is inscribed in
# that area, otherwise a launcher with a round mask cuts the letters of the
# piece in half.
ADAPTIVE_DP = 108
ADAPTIVE_VISIBLE_DP = 72

# The share of the side of the icon that the rounded corners take.
CORNER_RATIO = 0.22

# The wordmark of the lockup.
WORDMARK = "Trap"
# The font of the wordmark.  It is a subset of DejaVu Sans Bold that carries
# the glyphs of the wordmark only; it is part of the repository, so that the
# lockup is the same on every machine and the check cannot drift.
FONT = HERE / "fonts" / "Trap-Bold.ttf"

# The ink of the wordmark.
INK = (0x14, 0x11, 0x0E)


# ------------------------------------------------------------------- the bundle


def _read(relative: str) -> bytes:
    """The bytes of a file of the bundle."""
    path = BUNDLE / relative
    if not path.exists():
        raise SystemExit(f"the icon bundle is incomplete: {path} is missing")
    return path.read_bytes()


def _open(relative: str) -> Image.Image:
    """The picture of a file of the bundle."""
    return Image.open(io.BytesIO(_read(relative))).convert("RGBA")


# --------------------------------------------------------------- the operations


def rounded(image: Image.Image, ratio: float = CORNER_RATIO) -> Image.Image:
    """Round the corners of a picture."""
    width, height = image.size
    radius = int(round(min(width, height) * ratio))
    mask = Image.new("L", (width, height), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, width - 1, height - 1), radius=radius, fill=255
    )
    out = image.copy()
    out.putalpha(mask)
    return out


def circular(image: Image.Image) -> Image.Image:
    """Cut a picture into a circle."""
    width, height = image.size
    mask = Image.new("L", (width, height), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, width - 1, height - 1), fill=255)
    out = image.copy()
    out.putalpha(mask)
    return out


def fitted(image: Image.Image, canvas: int) -> Image.Image:
    """Place a picture in the safe zone of an adaptive icon.

    The launcher masks the canvas of 108dp down to the central 72dp and the
    shape it uses is a circle on many devices, so the picture is scaled to
    the square that is inscribed in that circle and centred.
    """
    side = int(
        round(canvas * ADAPTIVE_VISIBLE_DP / ADAPTIVE_DP / math.sqrt(2))
    )
    art = image.resize((side, side), Image.LANCZOS)
    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    out.alpha_composite(art, ((canvas - side) // 2, (canvas - side) // 2))
    return out


# ------------------------------------------------------------------- lockups


def _font(size: int) -> ImageFont.FreeTypeFont:
    """The font of the wordmark: the subset that is part of the repository."""
    if not FONT.exists():
        raise SystemExit(f"the font of the wordmark is missing: {FONT}")
    return ImageFont.truetype(str(FONT), size)


def lockup_icon(size: int) -> Image.Image:
    """The icon of the lockup: the artwork in a rounded square."""
    art = _open("ic_launcher_1024.png").resize((size, size), Image.LANCZOS)
    return rounded(art)


def lockup_png(width: int = 1024) -> Image.Image:
    """The icon next to the wordmark."""
    height = int(round(width * 0.28))
    icon = lockup_icon(height)
    font = _font(int(height * 0.40))
    text_width = int(font.getlength(WORDMARK))
    gap = int(height * 0.16)
    canvas = Image.new(
        "RGBA", (height + gap + text_width + int(height * 0.1), height), (0, 0, 0, 0)
    )
    canvas.alpha_composite(icon, (0, 0))
    draw = ImageDraw.Draw(canvas)
    box = draw.textbbox((0, 0), WORDMARK, font=font)
    draw.text(
        (height + gap, (height - (box[3] - box[1])) / 2 - box[1]),
        WORDMARK,
        font=font,
        fill=INK + (255,),
    )
    return canvas


def lockup_svg() -> str:
    """The lockup as a vector, with the icon embedded as a raster."""
    icon = lockup_icon(256)
    buffer = io.BytesIO()
    icon.save(buffer, format="PNG", optimize=True)
    encoded = base64.b64encode(buffer.getvalue()).decode("ascii")
    font = base64.b64encode(FONT.read_bytes()).decode("ascii")
    radius = int(round(256 * CORNER_RATIO))
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="1024" height="256" viewBox="0 0 1024 256">
  <title>{WORDMARK}</title>
  <defs>
    <clipPath id="round">
      <rect x="0" y="0" width="256" height="256" rx="{radius}"/>
    </clipPath>
    <style>
      @font-face {{
        font-family: "Trap";
        src: url("data:font/ttf;base64,{font}") format("truetype");
      }}
      .wordmark {{ font-family: "Trap", sans-serif; font-weight: bold; }}
    </style>
  </defs>
  <g clip-path="url(#round)">
    <image x="0" y="0" width="256" height="256"
           xlink:href="data:image/png;base64,{encoded}"/>
  </g>
  <text x="296" y="166" class="wordmark" font-size="102" fill="#14110E">{WORDMARK}</text>
</svg>
"""


# ------------------------------------------------------------------- writing


def render(target_res: Path, target_out: Path) -> dict[Path, bytes]:
    """Render every icon and return the files with their content."""
    files: dict[Path, bytes] = {}

    def put(path: Path, data: bytes) -> None:
        files[path] = data

    def put_png(path: Path, image: Image.Image) -> None:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", optimize=True)
        put(path, buffer.getvalue())

    # The launcher icons of every density are the ones of the bundle.
    for density in DENSITIES:
        put(
            target_res / f"mipmap-{density}" / "ic_launcher.png",
            _read(f"mipmap-{density}/ic_launcher.png"),
        )
        # The bundle carries the round variant of the icon, but it is the
        # square picture; the round one is cut out of the same artwork here.
        put_png(
            target_res / f"mipmap-{density}" / "ic_launcher_round.png",
            circular(_open(f"mipmap-{density}/ic_launcher.png")),
        )

    # The adaptive icon: the artwork inside the safe zone, the black field
    # behind it, and the silhouette for the themed icons of Android 13.
    put_png(
        target_res / "mipmap-xxxhdpi" / "ic_launcher_foreground.png",
        fitted(_open("adaptive/ic_launcher_foreground_432.png"), 432),
    )
    put(
        target_res / "mipmap-xxxhdpi" / "ic_launcher_background.png",
        _read("adaptive/ic_launcher_background_432.png"),
    )
    put_png(
        target_res / "mipmap-xxxhdpi" / "ic_launcher_monochrome.png",
        fitted(_open("adaptive/ic_launcher_monochrome_432.png"), 432),
    )

    # The mark of the interface: the artwork in a rounded square.
    put_png(
        target_res / "drawable-nodpi" / "ov_trap_mark.png",
        rounded(_open("play_store_512.png")),
    )

    # The icon of the listing and the exports of the themed icon.
    put(target_out / "play-store-1024.png", _read("ic_launcher_1024.png"))
    put(target_out / "play-store-512.png", _read("play_store_512.png"))
    put(
        target_out / "ic_launcher_monochrome_192.png",
        _read("ic_launcher_monochrome_192.png"),
    )
    put(
        target_out / "ic_launcher_monochrome_512.png",
        _read("ic_launcher_monochrome_512.png"),
    )

    # The wordmark of the documentation.
    put_png(target_out / "trap-lockup.png", lockup_png())
    put(target_out / "trap-lockup.svg", lockup_svg().encode())

    return files


def stale_files() -> list[Path]:
    """The files of the earlier artwork, which the bundle replaces."""
    paths = [RES / "drawable" / "ic_launcher_background.xml"]
    for density in DENSITIES:
        if density == "xxxhdpi":
            continue
        for name in ("ic_launcher_foreground.png", "ic_launcher_monochrome.png"):
            paths.append(RES / f"mipmap-{density}" / name)
    return paths


def unexpected_files(expected: set[Path]) -> list[Path]:
    """The icon files of the tree that the pipeline does not write."""
    found: list[Path] = []
    for directory in sorted(RES.glob("mipmap-*")):
        if not directory.is_dir():
            continue
        for path in sorted(directory.glob("ic_launcher*.png")):
            if path not in expected:
                found.append(path)
    for path in sorted(RES.glob("drawable/ic_launcher*")):
        if path not in expected:
            found.append(path)
    return found


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the files of the tree with a fresh rendering",
    )
    arguments = parser.parse_args(argv)

    if not BUNDLE.exists():
        raise SystemExit(f"the icon bundle is missing: {BUNDLE}")

    if arguments.check:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            files = render(root / "res", root / "out")
            stale: list[str] = []
            expected_res: set[Path] = set()
            for path, data in sorted(files.items()):
                relative = path.relative_to(root)
                current = (
                    (RES / relative.relative_to("res"))
                    if relative.parts[0] == "res"
                    else (OUT / relative.relative_to("out"))
                )
                if relative.parts[0] == "res":
                    expected_res.add(current)
                if not current.exists() or current.read_bytes() != data:
                    stale.append(str(current.relative_to(ROOT)))
            for path in stale_files():
                if path.exists():
                    stale.append(str(path.relative_to(ROOT)))
            for path in unexpected_files(expected_res):
                stale.append(str(path.relative_to(ROOT)))
            if stale:
                print("the icon lockups drifted:", file=sys.stderr)
                for name in sorted(set(stale)):
                    print(f"  {name}", file=sys.stderr)
                return 1
        print("the icon lockups are up to date")
        return 0

    files = render(RES, OUT)
    for path, data in sorted(files.items()):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    for path in stale_files():
        if path.exists():
            path.unlink()
    print(f"wrote {len(files)} files")
    for path in sorted(files):
        print(f"  {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
