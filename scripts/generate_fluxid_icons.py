"""Generate three fluxid app-icon variants: upright glossy droplet + bold EKG pulse."""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
OUT_DIR = Path(r"C:\Users\Matth\GrokAssets\fluxid")

CHARCOAL = (38, 40, 44, 255)
CHARCOAL_INNER = (48, 50, 55, 255)
BLUE = (0, 168, 255, 255)
BLUE_DARK = (0, 120, 200, 255)
CYAN_PULSE = (140, 230, 255, 255)
WHITE = (255, 255, 255, 255)
WHITE_SOFT = (235, 248, 255, 255)


def droplet_points(cx: float, cy: float, height: float, n: int = 240) -> list[tuple[float, float]]:
    """Upright teardrop: pointed top, wide rounded bottom."""
    w = height * 0.42
    h = height * 0.48
    points: list[tuple[float, float]] = []
    for i in range(n):
        t = 2 * math.pi * i / n
        x = cx + w * math.sin(t)
        y = cy + h * (-math.cos(t) + 0.55 * (1 + math.sin(t)) * (math.sin(t / 2) ** 2))
        points.append((x, y))
    return points


def ekg_points(cx: float, cy: float, width: float, amp: float) -> list[tuple[float, float]]:
    """Bold heartbeat trace matching concept-pulse rhythm."""
    profile = [
        (-0.50, 0.00),
        (-0.36, 0.00),
        (-0.30, 0.08),
        (-0.26, -0.06),
        (-0.22, 0.62),
        (-0.18, -0.52),
        (-0.14, 0.18),
        (-0.10, 0.00),
        (-0.06, 0.00),
        (-0.03, -0.12),
        (0.00, 0.28),
        (0.04, 0.00),
        (0.08, 0.00),
        (0.12, -0.08),
        (0.16, 0.40),
        (0.20, -0.30),
        (0.24, 0.00),
        (0.50, 0.00),
    ]
    return [(cx + u * width, cy + v * amp) for u, v in profile]


def draw_poly(draw: ImageDraw.ImageDraw, points: list[tuple[float, float]], fill=None, outline=None, width: int = 1):
    draw.polygon(points, fill=fill, outline=outline, width=width)


def draw_ekg(draw: ImageDraw.ImageDraw, points: list[tuple[float, float]], color: tuple[int, ...], width: int):
    if len(points) < 2:
        return
    draw.line(points, fill=color, width=width, joint="curve")
    r = max(2, width // 2)
    for p in (points[0], points[-1]):
        draw.ellipse((p[0] - r, p[1] - r, p[0] + r, p[1] + r), fill=color)


def apply_gloss(layer: Image.Image, droplet_pts: list[tuple[float, float]], cx: float, cy: float, height: float) -> Image.Image:
    mask = Image.new("L", layer.size, 0)
    mdraw = ImageDraw.Draw(mask)
    mdraw.polygon(droplet_pts, fill=255)

    gloss = Image.new("RGBA", layer.size, (0, 0, 0, 0))
    hdraw = ImageDraw.Draw(gloss)

    # Diagonal highlight streak (upper-left, like concept-droplet-graph)
    streak = Image.new("RGBA", layer.size, (0, 0, 0, 0))
    sdraw = ImageDraw.Draw(streak)
    sx = cx - height * 0.14
    sy = cy - height * 0.30
    sw = height * 0.10
    sh = height * 0.55
    sdraw.ellipse((sx, sy, sx + sw, sy + sh), fill=(255, 255, 255, 110))
    sdraw.ellipse((sx + sw * 0.15, sy + sh * 0.08, sx + sw * 0.75, sy + sh * 0.55), fill=(200, 235, 255, 55))
    streak = streak.filter(ImageFilter.GaussianBlur(radius=6))
    gloss = Image.alpha_composite(gloss, streak)

    # Small white dot highlight near top interior
    hdraw.ellipse(
        (cx - height * 0.04, cy - height * 0.18, cx + height * 0.06, cy - height * 0.08),
        fill=(255, 255, 255, 180),
    )

    # Subtle lower-left shadow/reflection
    bottom = Image.new("RGBA", layer.size, (0, 0, 0, 0))
    bdraw = ImageDraw.Draw(bottom)
    bdraw.ellipse(
        (cx - height * 0.10, cy + height * 0.08, cx + height * 0.22, cy + height * 0.30),
        fill=(0, 80, 140, 45),
    )
    gloss = Image.alpha_composite(gloss, bottom)
    gloss.putalpha(Image.composite(gloss.split()[3], Image.new("L", layer.size, 0), mask))
    return Image.alpha_composite(layer, gloss)


def base_tile() -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.rounded_rectangle((32, 32, SIZE - 33, SIZE - 33), radius=200, fill=CHARCOAL)
    inner = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    idraw = ImageDraw.Draw(inner)
    idraw.rounded_rectangle((48, 48, SIZE - 49, SIZE - 49), radius=188, fill=CHARCOAL_INNER)
    return Image.alpha_composite(img, inner)


def render_glossy_droplet() -> tuple[Image.Image, float, float, float, list[tuple[float, float]]]:
    cx, cy = SIZE / 2, SIZE / 2 + 28
    height = 510
    droplet = droplet_points(cx, cy, height)

    body = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    body_draw = ImageDraw.Draw(body)
    draw_poly(body_draw, droplet, fill=BLUE, outline=BLUE_DARK, width=3)
    body = apply_gloss(body, droplet, cx, cy, height)
    return body, cx, cy, height, droplet


def render_variant_1() -> Image.Image:
    """Glossy droplet + bright white bold pulse."""
    img = base_tile()
    body, cx, cy, height, _ = render_glossy_droplet()
    img = Image.alpha_composite(img, body)

    pulse_layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    pulse_draw = ImageDraw.Draw(pulse_layer)
    pulse = ekg_points(cx, cy + 4, width=height * 0.70, amp=height * 0.22)
    draw_ekg(pulse_draw, pulse, WHITE, width=28)
    return Image.alpha_composite(img, pulse_layer)


def render_variant_2() -> Image.Image:
    """Glossy droplet + lighter cyan-blue bold pulse."""
    img = base_tile()
    body, cx, cy, height, _ = render_glossy_droplet()
    img = Image.alpha_composite(img, body)

    pulse_layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    pulse_draw = ImageDraw.Draw(pulse_layer)
    pulse = ekg_points(cx, cy + 4, width=height * 0.70, amp=height * 0.22)
    draw_ekg(pulse_draw, pulse, CYAN_PULSE, width=28)
    return Image.alpha_composite(img, pulse_layer)


def render_variant_3() -> Image.Image:
    """Bolder minimal flat: strong outlines, no gloss streak, maximum small-size legibility."""
    img = base_tile()
    layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)

    cx, cy = SIZE / 2, SIZE / 2 + 28
    height = 500
    droplet = droplet_points(cx, cy, height)

    draw_poly(draw, droplet, fill=BLUE, outline=(0, 210, 255, 255), width=14)

    pulse = ekg_points(cx, cy + 2, width=height * 0.68, amp=height * 0.24)
    draw_ekg(draw, pulse, WHITE, width=40)

    return Image.alpha_composite(img, layer)


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    variants = [
        ("drop-pulse-up-1.png", render_variant_1),
        ("drop-pulse-up-2.png", render_variant_2),
        ("drop-pulse-up-3.png", render_variant_3),
    ]
    saved: list[Path] = []
    for name, fn in variants:
        out = OUT_DIR / name
        img = fn()
        img.save(out, "PNG")
        with Image.open(out) as check:
            assert check.size == (SIZE, SIZE), f"{name} wrong size: {check.size}"
        saved.append(out.resolve())
        print(f"Saved: {out.resolve()} ({out.stat().st_size} bytes, {SIZE}x{SIZE})")

    print("\nAll three variants saved:")
    for p in saved:
        print(f"  - {p}")


if __name__ == "__main__":
    main()