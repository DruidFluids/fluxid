"""Make corner background transparent on fluxid app icon, preserving tile content."""

from __future__ import annotations

from collections import deque
from pathlib import Path

from PIL import Image

SIZE = 1024
SRC = Path(r"C:\Users\Matth\GrokAssets\fluxid\YnDG8.jpg")
OUT = Path(r"C:\Users\Matth\GrokAssets\fluxid\icon-final.png")


def luminance(r: int, g: int, b: int) -> float:
    return 0.299 * r + 0.587 * g + 0.114 * b


def saturation(r: int, g: int, b: int) -> float:
    mx = max(r, g, b)
    mn = min(r, g, b)
    return (mx - mn) / mx if mx else 0.0


def is_outer_background(r: int, g: int, b: int) -> bool:
    """White corner fill and gray anti-alias fringe outside the dark tile."""
    lum = luminance(r, g, b)
    sat = saturation(r, g, b)
    # Bright neutral pixels: white corners + gray edge fringe.
    # Excludes blue droplet and cyan pulse (colored), and charcoal tile (dark).
    return lum >= 115 and sat <= 0.18


def flood_outer_background(img: Image.Image) -> set[tuple[int, int]]:
    """Flood-fill from corners through connected outer background pixels."""
    w, h = img.size
    rgb = img.convert("RGB")
    visited: set[tuple[int, int]] = set()
    queue: deque[tuple[int, int]] = deque(
        [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]
    )

    while queue:
        x, y = queue.popleft()
        if (x, y) in visited or x < 0 or y < 0 or x >= w or y >= h:
            continue
        r, g, b = rgb.getpixel((x, y))
        if not is_outer_background(r, g, b):
            continue
        visited.add((x, y))
        queue.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)])

    return visited


def apply_transparency(img: Image.Image, background: set[tuple[int, int]]) -> Image.Image:
    out = img.convert("RGBA")
    px = out.load()
    for x, y in background:
        r, g, b, _ = px[x, y]
        px[x, y] = (r, g, b, 0)
    return out


def verify(path: Path, src_rgb: Image.Image) -> dict:
    with Image.open(path) as img:
        assert img.mode == "RGBA", f"expected RGBA, got {img.mode}"
        assert img.size == (SIZE, SIZE), f"expected {SIZE}x{SIZE}, got {img.size}"
        px = img.load()
        corners = {
            "top_left": px[0, 0][3],
            "top_right": px[SIZE - 1, 0][3],
            "bottom_left": px[0, SIZE - 1][3],
            "bottom_right": px[SIZE - 1, SIZE - 1][3],
        }
        center = px[SIZE // 2, SIZE // 2]
        tile_edge = px[80, 80]
        highlight = px[480, 380]

        mismatches = 0
        for y in range(SIZE):
            for x in range(SIZE):
                if px[x, y][3] == 0:
                    continue
                sr, sg, sb = src_rgb.getpixel((x, y))
                or_, og, ob, _ = px[x, y]
                if (sr, sg, sb) != (or_, og, ob):
                    mismatches += 1

        # Ensure no opaque background fringe remains in corner zones.
        fringe_opaque = 0
        zones = [
            ((0, 0), 180),
            ((SIZE - 1, 0), 180),
            ((0, SIZE - 1), 180),
            ((SIZE - 1, SIZE - 1), 180),
        ]
        for (cx, cy), limit in zones:
            for y in range(max(0, cy - limit), min(SIZE, cy + limit + 1)):
                for x in range(max(0, cx - limit), min(SIZE, cx + limit + 1)):
                    if abs(x - cx) + abs(y - cy) > limit:
                        continue
                    r, g, b, a = px[x, y]
                    if a == 0:
                        continue
                    if is_outer_background(r, g, b):
                        fringe_opaque += 1

        return {
            "mode": img.mode,
            "size": img.size,
            "corner_alpha": corners,
            "center_rgba": center,
            "tile_edge_rgba": tile_edge,
            "highlight_rgba": highlight,
            "corners_transparent": all(a == 0 for a in corners.values()),
            "content_opaque": center[3] > 200 and tile_edge[3] > 200 and highlight[3] > 200,
            "rgb_mismatches": mismatches,
            "fringe_opaque_in_corners": fringe_opaque,
        }


def main() -> None:
    with Image.open(SRC) as raw:
        if raw.size != (SIZE, SIZE):
            raw = raw.resize((SIZE, SIZE), Image.Resampling.LANCZOS)
        src_rgb = raw.convert("RGB")
        img = raw.convert("RGBA")

    background = flood_outer_background(img)
    result = apply_transparency(img, background)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    result.save(OUT, "PNG")

    stats = verify(OUT, src_rgb)
    print(f"Saved: {OUT.resolve()}")
    print(f"  Size: {stats['size']}, Mode: {stats['mode']}")
    print(f"  Background pixels made transparent: {len(background)}")
    print(f"  Corner alpha: {stats['corner_alpha']}")
    print(f"  Center RGBA (droplet): {stats['center_rgba']}")
    print(f"  Highlight RGBA (streak): {stats['highlight_rgba']}")
    print(f"  Tile edge RGBA: {stats['tile_edge_rgba']}")
    print(f"  Corners fully transparent: {stats['corners_transparent']}")
    print(f"  Icon content preserved (opaque): {stats['content_opaque']}")
    print(f"  RGB mismatches on opaque pixels: {stats['rgb_mismatches']}")
    print(f"  Opaque background fringe in corners: {stats['fringe_opaque_in_corners']}")

    if not stats["corners_transparent"]:
        raise SystemExit("ERROR: corners are not fully transparent")
    if not stats["content_opaque"]:
        raise SystemExit("ERROR: icon content was damaged")
    if stats["rgb_mismatches"]:
        raise SystemExit("ERROR: opaque pixels differ from source")
    if stats["fringe_opaque_in_corners"]:
        raise SystemExit("ERROR: background fringe remains in corner zones")


if __name__ == "__main__":
    main()