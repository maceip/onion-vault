#!/usr/bin/env python3
"""Generate every PlugTalk icon lockup from one piece of artwork.

    python3 design/icons/plugtalk_icon.py           # write the files
    python3 design/icons/plugtalk_icon.py --check   # fail when they drifted

The artwork in `src/plugtalk-onion.png` is the single source: a smiling onion
on a teal badge.  The script cuts the onion out of the badge, and composes it
on the palette of the brand for every lockup, so the launcher icon, the
adaptive icon, the themed icon, the store icon and the wordmark can never
drift apart.

The output is byte for byte reproducible, and `--check` compares the files in
the tree with a fresh rendering, which is what the pipeline of the continuous
integration runs.

Only Pillow and NumPy are needed; both are in the image of the builder.
"""

from __future__ import annotations

import argparse
import base64
import io
import sys
import tempfile
from collections import deque
from pathlib import Path

try:
    import numpy as np
    from PIL import Image, ImageDraw, ImageFont
except ImportError as exc:  # pragma: no cover - the dependency is documented
    raise SystemExit(f"plugtalk_icon.py needs Pillow and NumPy: {exc}")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SRC = HERE / "src" / "plugtalk-onion.png"
OUT = HERE / "out"
RES = ROOT / "android" / "app" / "src" / "main" / "res"

# The palette of the brand: the teal to cyan of the badge of the artwork and
# the amber of the interface.
TEAL = (0x1D, 0xE5, 0xC2)
CYAN = (0x16, 0xC2, 0xCF)
DEEP = (0x10, 0xA1, 0xD8)
INK = (0x14, 0x11, 0x0E)
AMBER = (0xF5, 0xA9, 0x3C)

# The density buckets of the launcher icons: the canvas of the adaptive icon
# is 108dp and the artwork has to stay inside the central 72dp.
DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}
ADAPTIVE_DP = 108
LAUNCHER_DP = 48

# The wordmark of the lockup.
WORDMARK = "PlugTalk"
# The font of the wordmark.  It is a subset of DejaVu Sans Bold that carries
# the glyphs of the wordmark only; it is part of the repository, so that the
# lockup is the same on every machine and the check cannot drift.
FONT = HERE / "fonts" / "PlugTalk-Bold.ttf"


# ---------------------------------------------------------------- extraction


def _label(mask: np.ndarray) -> tuple[np.ndarray, int]:
    """Label the connected components of a boolean mask (4-neighbourhood)."""
    height, width = mask.shape
    labels = np.zeros((height, width), np.int32)
    current = 0
    for start_y in range(height):
        for start_x in range(width):
            if not mask[start_y, start_x] or labels[start_y, start_x]:
                continue
            current += 1
            queue = deque([(start_y, start_x)])
            labels[start_y, start_x] = current
            while queue:
                y, x = queue.popleft()
                for ny, nx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                    if 0 <= ny < height and 0 <= nx < width:
                        if mask[ny, nx] and not labels[ny, nx]:
                            labels[ny, nx] = current
                            queue.append((ny, nx))
    return labels, current


def _fill_holes(mask: np.ndarray) -> np.ndarray:
    """Fill every hole of the mask that does not touch its border."""
    height, width = mask.shape
    outside = np.zeros((height, width), bool)
    queue: deque[tuple[int, int]] = deque()
    for x in range(width):
        for y in (0, height - 1):
            if not mask[y, x] and not outside[y, x]:
                outside[y, x] = True
                queue.append((y, x))
    for y in range(height):
        for x in (0, width - 1):
            if not mask[y, x] and not outside[y, x]:
                outside[y, x] = True
                queue.append((y, x))
    while queue:
        y, x = queue.popleft()
        for ny, nx in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
            if 0 <= ny < height and 0 <= nx < width:
                if not mask[ny, nx] and not outside[ny, nx]:
                    outside[ny, nx] = True
                    queue.append((ny, nx))
    return mask | ~outside


def cut_mark(path: Path) -> Image.Image:
    """Cut the onion out of the teal badge of the artwork.

    The badge is a smooth gradient, so the background of the badge is modelled
    with a quadratic per colour channel, fitted with robust iterations that
    trim the onion away.  What is left of the picture is the onion.
    """
    picture = Image.open(path).convert("RGB")
    pixels = np.array(picture).astype(float)
    height, width = pixels.shape[:2]
    grid_y, grid_x = np.mgrid[0:height, 0:width]

    # The white page around the badge is the only part of the picture that is
    # not part of the field.
    field = pixels.min(axis=2) <= 215

    def design(mask: np.ndarray) -> np.ndarray:
        return np.stack(
            [
                np.ones(int(mask.sum())),
                grid_x[mask],
                grid_y[mask],
                (grid_x * grid_y)[mask],
                (grid_x * grid_x)[mask],
                (grid_y * grid_y)[mask],
            ],
            1,
        ).astype(float)

    full = np.stack(
        [
            np.ones(height * width),
            grid_x.ravel(),
            grid_y.ravel(),
            (grid_x * grid_y).ravel(),
            (grid_x * grid_x).ravel(),
            (grid_y * grid_y).ravel(),
        ],
        1,
    ).astype(float)

    keep = field.copy()
    coefficients = None
    for _ in range(15):
        matrix = design(keep)
        coefficients, *_ = np.linalg.lstsq(matrix, pixels[keep], rcond=None)
        model = (full @ coefficients).reshape(height, width, 3)
        residual = np.abs(pixels - model).max(axis=2)
        limit = max(10.0, float(np.percentile(residual[keep], 75)))
        keep = field & (residual < limit)

    model = (full @ coefficients).reshape(height, width, 3)
    residual = np.abs(pixels - model).max(axis=2)

    # The onion is everything that the model of the badge cannot explain.
    foreground = field & (residual > 60)
    labels, _ = _label(foreground)
    sizes = np.bincount(labels.ravel())
    sizes[0] = 0
    order = np.argsort(sizes)[::-1]
    keep_labels = [int(index) for index in order[:4] if sizes[index] > 400]
    mark = _fill_holes(np.isin(labels, keep_labels))

    cut = np.zeros((height, width, 4), np.uint8)
    cut[..., :3] = np.clip(pixels, 0, 255).astype(np.uint8)
    cut[..., 3] = (mark * 255).astype(np.uint8)
    return Image.fromarray(cut, "RGBA")


def trim(image: Image.Image) -> Image.Image:
    """Cut the transparent border of an image."""
    box = image.getchannel("A").getbbox()
    return image.crop(box) if box else image


# --------------------------------------------------------------- composition


def brand_gradient(size: tuple[int, int]) -> Image.Image:
    """The gradient of the badge of the artwork, from the teal to the cyan."""
    width, height = size
    x = np.linspace(0.0, 1.0, width)[None, :]
    y = np.linspace(0.0, 1.0, height)[:, None]
    # The same direction as the gradient of the adaptive icon: from the top
    # left to the bottom right.
    position = np.clip(0.5 + (x - y) / 2.0, 0.0, 1.0)
    stops = np.array([TEAL, CYAN, DEEP], float)
    scale = position * (len(stops) - 1)
    low = np.clip(np.floor(scale).astype(int), 0, len(stops) - 2)
    high = low + 1
    mix = (scale - low)[..., None]
    canvas = stops[low] * (1.0 - mix) + stops[high] * mix
    return Image.fromarray(canvas.astype(np.uint8), "RGB")


def place_mark(
    canvas: Image.Image,
    mark: Image.Image,
    height_ratio: float,
    centre: tuple[float, float] = (0.5, 0.5),
) -> Image.Image:
    """Draw the mark on the canvas, scaled to the given share of its height."""
    width, height = canvas.size
    target = max(1, int(round(height * height_ratio)))
    scale = target / mark.height
    scaled = mark.resize(
        (max(1, int(round(mark.width * scale))), target), Image.LANCZOS
    )
    x = int(round(width * centre[0] - scaled.width / 2))
    y = int(round(height * centre[1] - scaled.height / 2))
    if canvas.mode == "RGB":
        canvas = canvas.convert("RGBA")
    canvas.alpha_composite(scaled, (x, y))
    return canvas


def hero_icon(mark: Image.Image, size: int) -> Image.Image:
    """The launcher icon: the mark on the gradient of the brand."""
    canvas = brand_gradient((size, size)).convert("RGBA")
    return place_mark(canvas, mark, 0.76, (0.5, 0.52))


def round_icon(mark: Image.Image, size: int) -> Image.Image:
    """The icon of the launchers that ask for a round icon."""
    canvas = brand_gradient((size, size)).convert("RGBA")
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)
    canvas = place_mark(canvas, mark, 0.56, (0.5, 0.52))
    canvas.putalpha(mask)
    return canvas


def adaptive_foreground(mark: Image.Image, canvas: int) -> Image.Image:
    """The artwork of the adaptive icon, inside the safe zone."""
    image = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    # The safe zone of the adaptive icon is the central 72dp of the 108dp
    # canvas, so the artwork never grows past two thirds of the canvas.
    return place_mark(image, mark, 0.56)


def monochrome(mark: Image.Image, canvas: int) -> Image.Image:
    """The silhouette of the mark, for the themed icons of Android 13."""
    silhouette = Image.new("RGBA", mark.size, (255, 255, 255, 0))
    silhouette.putalpha(mark.getchannel("A"))
    image = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    return place_mark(image, silhouette, 0.56)


# ------------------------------------------------------------------ lockups


def _font(size: int) -> ImageFont.FreeTypeFont:
    """The font of the wordmark: the subset that is part of the repository."""
    if not FONT.exists():
        raise SystemExit(f"the font of the wordmark is missing: {FONT}")
    return ImageFont.truetype(str(FONT), size)


def lockup_png(mark: Image.Image, width: int = 1024) -> Image.Image:
    """The icon next to the wordmark."""
    height = int(round(width * 0.28))
    icon = hero_icon(mark, height)
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


def lockup_svg(mark: Image.Image) -> str:
    """The lockup as a vector, with the icon embedded as a raster."""
    icon = hero_icon(mark, 256)
    buffer = io.BytesIO()
    icon.save(buffer, format="PNG", optimize=True)
    encoded = base64.b64encode(buffer.getvalue()).decode("ascii")
    font = base64.b64encode(FONT.read_bytes()).decode("ascii")
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="1024" height="256" viewBox="0 0 1024 256">
  <title>{WORDMARK}</title>
  <defs>
    <linearGradient id="brand" x1="0" y1="0" x2="0.707" y2="0.707">
      <stop offset="0" stop-color="#1DE5C2"/>
      <stop offset="0.5" stop-color="#16C2CF"/>
      <stop offset="1" stop-color="#10A1D8"/>
    </linearGradient>
    <clipPath id="round">
      <rect x="0" y="0" width="256" height="256" rx="58"/>
    </clipPath>
    <style>
      @font-face {{
        font-family: "PlugTalk";
        src: url("data:font/ttf;base64,{font}") format("truetype");
      }}
      .wordmark {{ font-family: "PlugTalk", sans-serif; font-weight: bold; }}
    </style>
  </defs>
  <g clip-path="url(#round)">
    <rect x="0" y="0" width="256" height="256" fill="url(#brand)"/>
    <image x="0" y="0" width="256" height="256"
           xlink:href="data:image/png;base64,{encoded}"/>
  </g>
  <text x="296" y="166" class="wordmark" font-size="102" fill="#14110E">{WORDMARK}</text>
</svg>
"""


BACKGROUND_XML = """<?xml version="1.0" encoding="utf-8"?>
<!--
    The background of the adaptive icon of PlugTalk.

    It is the gradient of the badge of the artwork: teal at the top left,
    cyan at the bottom right.
-->
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <gradient
        android:angle="315"
        android:type="linear"
        android:startColor="#1DE5C2"
        android:centerColor="#16C2CF"
        android:endColor="#10A1D8" />
</shape>
"""


# ------------------------------------------------------------------- writing


def _save_png(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, format="PNG", optimize=True)


def render(target_res: Path, target_out: Path) -> dict[Path, bytes]:
    """Render every lockup and return the files with their content."""
    mark = trim(cut_mark(SRC))
    files: dict[Path, bytes] = {}

    def put(path: Path, data: bytes) -> None:
        files[path] = data

    def put_png(path: Path, image: Image.Image) -> None:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", optimize=True)
        put(path, buffer.getvalue())

    # The mark itself, for the about screen of the application.
    put_png(target_res / "drawable-nodpi" / "ov_plugtalk_mark.png", mark)

    for density, factor in DENSITIES.items():
        launcher = int(round(LAUNCHER_DP * factor))
        adaptive = int(round(ADAPTIVE_DP * factor))
        put_png(
            target_res / f"mipmap-{density}" / "ic_launcher.png",
            hero_icon(mark, launcher),
        )
        put_png(
            target_res / f"mipmap-{density}" / "ic_launcher_round.png",
            round_icon(mark, launcher),
        )
        put_png(
            target_res / f"mipmap-{density}" / "ic_launcher_foreground.png",
            adaptive_foreground(mark, adaptive),
        )
        put_png(
            target_res / f"mipmap-{density}" / "ic_launcher_monochrome.png",
            monochrome(mark, adaptive),
        )

    put(target_res / "drawable" / "ic_launcher_background.xml", BACKGROUND_XML.encode())

    # The store icon of the listing, and the wordmark of the documentation.
    for size in (512, 1024):
        put_png(target_out / f"play-store-{size}.png", hero_icon(mark, size))
    put_png(target_out / "plugtalk-lockup.png", lockup_png(mark))
    put(target_out / "plugtalk-lockup.svg", lockup_svg(mark).encode())

    return files


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the files of the tree with a fresh rendering",
    )
    arguments = parser.parse_args(argv)

    if not SRC.exists():
        raise SystemExit(f"the artwork is missing: {SRC}")

    if arguments.check:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            files = render(root / "res", root / "out")
            stale: list[str] = []
            for path, data in sorted(files.items()):
                relative = path.relative_to(root)
                current = (
                    (RES / relative.relative_to("res"))
                    if relative.parts[0] == "res"
                    else (OUT / relative.relative_to("out"))
                )
                if not current.exists() or current.read_bytes() != data:
                    stale.append(str(current.relative_to(ROOT)))
            if stale:
                print("the icon lockups drifted:", file=sys.stderr)
                for name in stale:
                    print(f"  {name}", file=sys.stderr)
                return 1
        print("the icon lockups are up to date")
        return 0

    files = render(RES, OUT)
    for path, data in sorted(files.items()):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    print(f"wrote {len(files)} files")
    for path in sorted(files):
        print(f"  {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
