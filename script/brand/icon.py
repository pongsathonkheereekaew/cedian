"""Writes the cedian icon and mark into brand/: seeds set at the golden angle, as a sunflower
sets them (phyllotaxis), the first seed in lime (owner, 2026-10-23).

Run from the repository root: python3 script/brand/icon.py
Needs Pillow. The SVGs are the source; the PNG, ICNS and ICO files are drawn here from the same
seeds, so nothing else has to rasterise an SVG.
"""

import math
import pathlib

from PIL import Image, ImageDraw

GOLDEN = math.radians(137.508)

# One seed per Fibonacci count; the small layout has fewer, larger seeds for 32 px and under.
FULL = {"count": 34, "spread": 7.1, "inner": 2.2, "outer": 5.2, "first": 1.35}
SMALL = {"count": 21, "spread": 9.2, "inner": 3.0, "outer": 6.6, "first": 1.8}

INK = "#151412"  # the tile, the dark theme's frame
SEED_DARK = "#EDEBE5"  # the dark theme's text
SEED_LIGHT = "#1C1C1A"  # the light theme's text
LIME_DARK = "#C6F432"  # accent, dark
LIME_LIGHT = "#5E8A00"  # accent, light


def seeds(layout):
    """(x, y, r, first) on a 100-unit square centred on 50, 50; outermost first."""
    out = []
    for i in range(layout["count"], 0, -1):
        a, d = i * GOLDEN, layout["spread"] * math.sqrt(i)
        r = layout["inner"] + (layout["outer"] - layout["inner"]) * math.sqrt(i / layout["count"])
        out.append((50 + d * math.cos(a), 50 + d * math.sin(a), r * layout["first"] if i == 1 else r, i == 1))
    return out


def circles(layout, seed, first):
    return "".join(
        f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" fill="{first if f else seed}"/>'
        for x, y, r, f in seeds(layout)
    )


def mark_svg(layout, seed, first):
    body = circles(layout, seed, first)
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="2 2 96 96" role="img" aria-label="Cedian">'
        f"<title>Cedian</title>{body}</svg>\n"
    )


# The macOS grid: an 824 px tile with 184.33 px corners on a 1024 px canvas; the seeds span 60%.
TILE, CORNER, SCALE = 824, 184.33, 5.3


def icon_svg(layout):
    body = circles(layout, SEED_DARK, LIME_DARK)
    shift = 512 - 50 * SCALE
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024"'
        ' role="img" aria-label="Cedian"><title>Cedian</title>'
        f'<rect x="100" y="100" width="{TILE}" height="{TILE}" rx="{CORNER}" fill="{INK}"/>'
        f'<g transform="translate({shift:.2f} {shift:.2f}) scale({SCALE})">{body}</g></svg>\n'
    )


def draw(size, layout, seed, first, tile, box=(2, 98)):
    """Draws at 4x and scales down, which is enough antialiasing for filled circles."""
    k = 4
    big = Image.new("RGBA", (size * k, size * k), (0, 0, 0, 0))
    g = ImageDraw.Draw(big)
    if tile:
        unit = size * k / 1024
        g.rounded_rectangle(
            [100 * unit, 100 * unit, (100 + TILE) * unit, (100 + TILE) * unit],
            radius=CORNER * unit,
            fill=INK,
        )
        px, off = SCALE * unit, (512 - 50 * SCALE) * unit
    else:
        px = size * k / (box[1] - box[0])
        off = -box[0] * px
    for x, y, r, f in seeds(layout):
        cx, cy, rr = off + x * px, off + y * px, r * px
        g.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=first if f else seed)
    return big.resize((size, size), Image.LANCZOS)


def app_icon(size):
    return draw(size, SMALL if size <= 32 else FULL, SEED_DARK, LIME_DARK, tile=True)


brand = pathlib.Path("brand")
marks = {
    "dark": (SEED_DARK, LIME_DARK),
    "light": (SEED_LIGHT, LIME_LIGHT),
    "mono-black": ("#0B0D10", "#0B0D10"),
    "mono-white": ("#FFFFFF", "#FFFFFF"),
}
for name, (seed, first) in marks.items():
    (brand / "mark" / f"cedian-mark-{name}.svg").write_text(mark_svg(FULL, seed, first))
    draw(1024, FULL, seed, first, tile=False).save(brand / "mark" / f"cedian-mark-{name}-1024.png")
(brand / "mark" / "cedian-mark-small-mono-black.svg").write_text(mark_svg(SMALL, "#0B0D10", "#0B0D10"))

(brand / "app-icon" / "macos-1024.svg").write_text(icon_svg(FULL))
app_icon(1024).save(brand / "app-icon" / "macos-1024.png")

sizes = [16, 32, 64, 128, 256, 512, 1024]
frames = {s: app_icon(s) for s in sizes}
frames[1024].save(brand / "AppIcon.icns", append_images=[frames[s] for s in sizes[:-1]])
ico = [16, 24, 32, 48, 256]
app_icon(256).save(
    brand / "app-icon" / "windows.ico",
    sizes=[(s, s) for s in ico],
    append_images=[app_icon(s) for s in ico[:-1]],
)
print("brand/")
