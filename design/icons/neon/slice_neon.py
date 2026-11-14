#!/usr/bin/env python3
"""Slice the neon icon system of Trap out of the design board.

    python3 design/icons/neon/slice_neon.py           # write the files
    python3 design/icons/neon/slice_neon.py --check   # fail when they drifted

The board in `design/icons/neon/trap-board.jpg` is the source: it carries the
logo, the icon system and the components of the browser chrome of the
application.

Every asset is cut out of the board with an alpha channel.  The search box of
an asset holds the glyph, the label that belongs to it and the scraps of the
panel around it; the biggest bright shape of the box is the glyph, and the
cut is made around it.  The alpha of a pixel is how far it is from the near
black of the board, so the neon glow fades out and the dark outline of a
glyph becomes the glow itself.  The parts that do not belong to the glyph,
such as the label of an icon, are dropped.

The output is byte for byte reproducible, and `--check` compares the files of
the tree with a fresh rendering, which is what the pipeline of the continuous
integration runs.

Only Pillow and NumPy are needed.
"""

from __future__ import annotations

import argparse
import io
import sys
import tempfile
from collections import deque
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BOARD = HERE / "trap-board.jpg"
RES = ROOT / "android" / "app" / "src" / "main" / "res" / "drawable-nodpi"

# The search box of every asset of the board: the name and the box in the
# coordinates of the board.
ASSETS: dict[str, tuple[int, int, int, int]] = {
    # The icon system: the boxes of the cells of the grid, without the label
    # that sits under every icon.
    "ic_neon_back": (18, 366, 112, 448),
    "ic_neon_forward": (150, 366, 246, 448),
    "ic_neon_reload": (280, 366, 376, 450),
    "ic_neon_autofill": (406, 366, 515, 448),
    "ic_neon_sign_pgp": (18, 468, 118, 545),
    "ic_neon_decrypt_pgp": (148, 468, 247, 545),
    "ic_neon_monero": (280, 468, 376, 545),
    "ic_neon_usd": (423, 468, 500, 545),
    "ic_neon_cancel": (22, 578, 107, 650),
    "ic_neon_new_tab": (143, 578, 246, 650),
    "ic_neon_new_partial_tab": (275, 578, 380, 650),
    "ic_neon_update": (413, 578, 509, 650),
    "ic_neon_menu": (17, 682, 111, 760),
    "ic_neon_microphone": (159, 682, 230, 760),
    "ic_neon_zoom_in": (280, 682, 372, 760),
    "ic_neon_zoom_out": (413, 682, 506, 760),
    # The components of the chrome.
    "ic_neon_shield": (560, 472, 602, 514),
    "ic_neon_mic": (968, 472, 1000, 514),
    "ic_neon_home": (670, 700, 718, 745),
    "ic_neon_wallet": (930, 632, 962, 658),
    "ic_neon_onion": (662, 630, 698, 660),
    "ic_neon_key": (828, 633, 858, 663),
    "ic_neon_dots": (986, 698, 1018, 748),
    # The brand of the application.  The box of the wordmark stays between
    # the two lines that frame the panel, so that the frame is not part of
    # the glyph.
    "ic_neon_trap_logo": (606, 378, 800, 443),
    "ic_neon_onion_twins": (804, 376, 900, 440),
}

# The near black of the board, which is also the near black of the brand.
BACKGROUND = np.array([21, 21, 29], float)

# The lavender of the board itself, around the panels.
LAVENDER = np.array([191, 160, 253], float)

# How far a pixel has to be from the background to be opaque, and where the
# glow fades out.
FLOOR = 12.0
CEILING = 58.0

# The brightness over which a pixel belongs to a glyph.
BRIGHT = 85.0

# A part of the cut that is smaller than this share of the biggest part is a
# scrap of the board (the label of an icon, the edge of a panel) and is
# dropped.
SHARE = 0.18

# The air that is kept around the glyph of an asset.
MARGIN = 4


def _components(mask: np.ndarray) -> list[np.ndarray]:
    """The connected components of a boolean mask."""
    height, width = mask.shape
    labels = np.zeros(mask.shape, np.int32)
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
                    if (
                        0 <= ny < height
                        and 0 <= nx < width
                        and mask[ny, nx]
                        and not labels[ny, nx]
                    ):
                        labels[ny, nx] = current
                        queue.append((ny, nx))
    return [labels == index for index in range(1, current + 1)]


def _glyph_box(solid: np.ndarray) -> tuple[int, int, int, int] | None:
    """The box of the bright shapes of a picture.

    A glyph is made of several bright shapes: the fill, the highlight and the
    glow around it.  They all belong together, so the box covers every shape
    that is as big as the biggest one; the letters of a label are much
    smaller and stay out.
    """
    parts = _components(solid)
    if not parts:
        return None
    biggest = max(part.sum() for part in parts)
    ys, xs = np.nonzero(
        np.logical_or.reduce([part for part in parts if part.sum() >= SHARE * biggest])
    )
    top, bottom = int(ys.min()), int(ys.max())
    left, right = int(xs.min()), int(xs.max())
    return (
        max(0, left - MARGIN),
        max(0, top - MARGIN),
        min(solid.shape[1], right + MARGIN),
        min(solid.shape[0], bottom + MARGIN),
    )


def _is_panel_edge(part: np.ndarray) -> bool:
    """`True` when a part of the cut is a line of the panel of the board.

    The board draws its panels with a thin bright line.  Such a line is long
    and flat, and it never belongs to a glyph.
    """
    ys, xs = np.nonzero(part)
    if ys.size == 0:
        return True
    height = int(ys.max()) - int(ys.min()) + 1
    width = int(xs.max()) - int(xs.min()) + 1
    return height <= 3 and width >= 20


def _drop_panel_lines(alpha: np.ndarray) -> np.ndarray:
    """Remove the horizontal lines that draw the panels of the board.

    The board frames its panels with a thin bright line that spans the whole
    width of a cell.  Such a band of rows, and the glow that hangs under it,
    are never part of a glyph.
    """
    solid = alpha > 96
    height, width = solid.shape
    if width == 0 or height == 0:
        return alpha
    full = solid.sum(axis=1) >= 0.92 * width

    top = 0
    while top < height and full[top]:
        top += 1
    if top:
        glow = top
        while (
            glow < height
            and not full[glow]
            and solid[glow].sum() > 0.3 * width
        ):
            glow += 1
        alpha[:glow, :] = 0

    bottom = height - 1
    while bottom >= 0 and full[bottom]:
        bottom -= 1
    if bottom < height - 1:
        glow = bottom
        while glow >= 0 and not full[glow] and solid[glow].sum() > 0.3 * width:
            glow -= 1
        alpha[glow + 1 :, :] = 0

    return alpha


def cut(board: Image.Image, box: tuple[int, int, int, int]) -> Image.Image:
    """Cut one asset out of the board."""
    crop = board.crop(box).convert("RGB")
    pixels = np.array(crop).astype(float)

    # The alpha of a pixel is how far it is from the near black of the board.
    distance = np.sqrt(((pixels - BACKGROUND) ** 2).sum(axis=2))
    lavender = np.sqrt(((pixels - LAVENDER) ** 2).sum(axis=2))
    alpha = np.clip((distance - FLOOR) / (CEILING - FLOOR), 0.0, 1.0)
    # The board itself is not part of a glyph.
    alpha[lavender < 48] = 0.0
    # The frames of the panels are not part of a glyph either.
    alpha = _drop_panel_lines(alpha)

    # The cut is made around the glyph itself.
    glyph = _glyph_box(alpha > 96)
    if glyph is not None:
        crop = crop.crop(glyph)
        pixels = np.array(crop).astype(float)
        distance = np.sqrt(((pixels - BACKGROUND) ** 2).sum(axis=2))
        lavender = np.sqrt(((pixels - LAVENDER) ** 2).sum(axis=2))
        alpha = np.clip((distance - FLOOR) / (CEILING - FLOOR), 0.0, 1.0)
        alpha[lavender < 48] = 0.0

    cut_out = np.zeros((crop.height, crop.width, 4), np.uint8)
    cut_out[..., :3] = np.clip(pixels, 0, 255).astype(np.uint8)
    cut_out[..., 3] = (alpha * 255).astype(np.uint8)
    image = Image.fromarray(cut_out, "RGBA")

    # Only the glyph survives: the label of an icon, the edge of a panel and
    # the lines of the panels are scraps of the board.
    solid = np.array(image.getchannel("A")) > 96
    parts = _components(solid)
    if parts:
        biggest = max(part.sum() for part in parts)
        keep = np.zeros(solid.shape, bool)
        for part in parts:
            if part.sum() >= SHARE * biggest and not _is_panel_edge(part):
                keep |= part
        cut_out[..., 3] = np.where(keep, cut_out[..., 3], 0).astype(np.uint8)
        image = Image.fromarray(cut_out, "RGBA")

    # The frames of the panels that are inside the cut are dropped.
    alpha = np.array(image.getchannel("A"))
    image.putalpha(Image.fromarray(_drop_panel_lines(alpha)))

    bounds = image.getchannel("A").getbbox()
    return image.crop(bounds) if bounds else image


def render(target: Path) -> dict[Path, bytes]:
    """Render every asset and return the files with their content."""
    board = Image.open(BOARD)
    files: dict[Path, bytes] = {}
    for name, box in ASSETS.items():
        image = cut(board, box)
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", optimize=True)
        files[target / f"{name}.png"] = buffer.getvalue()
    return files


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the files of the tree with a fresh rendering",
    )
    arguments = parser.parse_args(argv)

    if not BOARD.exists():
        raise SystemExit(f"the design board is missing: {BOARD}")

    if arguments.check:
        with tempfile.TemporaryDirectory() as temporary:
            files = render(Path(temporary))
            stale: list[str] = []
            for path, data in sorted(files.items()):
                current = RES / path.name
                if not current.exists() or current.read_bytes() != data:
                    stale.append(str(current.relative_to(ROOT)))
            if stale:
                print("the neon assets drifted:", file=sys.stderr)
                for name in stale:
                    print(f"  {name}", file=sys.stderr)
                return 1
        print("the neon assets are up to date")
        return 0

    files = render(RES)
    for path, data in sorted(files.items()):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    print(f"wrote {len(files)} files")
    for path in sorted(files):
        print(f"  {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
