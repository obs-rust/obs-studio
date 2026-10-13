#!/usr/bin/env python3
"""Regenerate every obs-rust logo asset from one source image.

Usage: uv run --with pillow python docs/images/generate-logo-assets.py SOURCE

SOURCE is the square logo on a plain light background. The ring's dark
outline is located along rays from the centre and a circle is fitted to
it, so the cut follows the real edge even when the artwork is off-centre:
the ring and its outline are kept, and everything outside is transparent.
"""

import base64
import io
import math
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
MASTER = 1024
OUTLINE_LEVEL = 215  # the outline is ~170 grey, the background ~254
RAYS = 720
SUPERSAMPLE = 4


def fit_circle(points):
    """Least-squares (Kasa) circle fit; returns (cx, cy, r)."""
    n = len(points)
    sx = sum(x for x, _ in points)
    sy = sum(y for _, y in points)
    sxx = sum(x * x for x, _ in points)
    syy = sum(y * y for _, y in points)
    sxy = sum(x * y for x, y in points)
    sz = [x * x + y * y for x, y in points]
    sxz = sum(x * z for (x, _), z in zip(points, sz))
    syz = sum(y * z for (_, y), z in zip(points, sz))
    szs = sum(sz)
    # Solve [sxx sxy sx; sxy syy sy; sx sy n] [a b c] = [sxz syz szs]
    m = [[sxx, sxy, sx, sxz], [sxy, syy, sy, syz], [sx, sy, n, szs]]
    for i in range(3):
        p = max(range(i, 3), key=lambda k: abs(m[k][i]))
        m[i], m[p] = m[p], m[i]
        for k in range(3):
            if k != i:
                f = m[k][i] / m[i][i]
                m[k] = [a - f * b for a, b in zip(m[k], m[i])]
    a, b, c = (m[i][3] / m[i][i] for i in range(3))
    cx, cy = a / 2, b / 2
    return cx, cy, math.sqrt(c + cx * cx + cy * cy)


def outline_points(gray):
    w, h = gray.size
    px = gray.load()
    cx, cy = (w - 1) / 2, (h - 1) / 2
    points = []
    for k in range(RAYS):
        a = 2 * math.pi * k / RAYS
        for r in range(int(math.hypot(w, h) / 2), 0, -1):
            x = int(round(cx + r * math.cos(a)))
            y = int(round(cy + r * math.sin(a)))
            if 0 <= x < w and 0 <= y < h and px[x, y] < OUTLINE_LEVEL:
                points.append((x, y))
                break
    return points


def cut_out(source):
    rgb = source.convert("RGB")
    cx, cy, r = fit_circle(outline_points(rgb.convert("L")))
    r += 1.0  # include the anti-aliased outer edge of the outline
    print(f"circle: centre=({cx:.1f}, {cy:.1f}) radius={r:.1f}")

    # Anti-aliased circular mask, drawn supersampled.
    w, h = rgb.size
    big = Image.new("L", (w * SUPERSAMPLE, h * SUPERSAMPLE), 0)
    s = SUPERSAMPLE
    ImageDraw.Draw(big).ellipse(
        [(cx + 0.5 - r) * s, (cy + 0.5 - r) * s, (cx + 0.5 + r) * s, (cy + 0.5 + r) * s],
        fill=255,
    )
    mask = big.resize((w, h), Image.LANCZOS)
    logo = rgb.convert("RGBA")
    logo.putalpha(mask)

    # Square crop centred on the circle, then scale to the master size.
    half = r + 1
    box = (cx + 0.5 - half, cy + 0.5 - half, cx + 0.5 + half, cy + 0.5 + half)
    return logo.resize((MASTER, MASTER), Image.LANCZOS, box=box)


def sized(master, size):
    return master.resize((size, size), Image.LANCZOS)


def save_png(master, size, rel):
    path = ROOT / rel
    sized(master, size).save(path, optimize=True)
    print("wrote", rel)


def save_ico(master, sizes, rel):
    path = ROOT / rel
    master.save(path, format="ICO", sizes=[(s, s) for s in sizes])
    print("wrote", rel)


def paused(master):
    """The Windows/Linux tray icon while recording is paused: yellow pause bars."""
    icon = sized(master, 256)
    draw = ImageDraw.Draw(icon)
    for x0, x1 in ((13, 53), (87, 127)):
        draw.rectangle([x0 - 4, 107, x1 + 4, 255], fill=(0, 0, 0, 255))
        draw.rectangle([x0, 111, x1, 255], fill=(255, 255, 0, 255))
    return icon


def active(master):
    """The Windows/Linux tray icon while live or recording: a red dot."""
    icon = sized(master, 256)
    draw = ImageDraw.Draw(icon)
    draw.ellipse([0, 148, 108, 256], fill=(0, 0, 0, 255))
    draw.ellipse([6, 154, 102, 250], fill=(255, 0, 0, 255))
    return icon


def mask(master):
    """The macOS menu bar icon: Qt draws it as a template (mask) image, so
    only alpha matters. Keep the light ring, swooshes and gear; drop the
    dark disc."""
    icon = sized(master, 256)
    alpha = icon.convert("L").point(lambda v: max(0, min(255, (v - 60) * 4)))
    alpha = Image.composite(alpha, Image.new("L", icon.size, 0), icon.getchannel("A"))
    white = Image.new("RGBA", icon.size, (255, 255, 255, 0))
    white.putalpha(alpha)
    return white


def mask_badge(icon, badge):
    """Cut a gap round a bottom-left badge on a mask icon, then draw it."""
    a = icon.getchannel("A")
    draw = ImageDraw.Draw(a)
    if badge == "dot":
        draw.ellipse([0, 148, 108, 256], fill=0)
        draw.ellipse([10, 158, 98, 246], fill=255)
    else:
        draw.rectangle([0, 103, 140, 256], fill=0)
        for x0, x1 in ((13, 53), (87, 127)):
            draw.rectangle([x0, 111, x1, 255], fill=255)
    icon.putalpha(a)
    return icon


def write_png_svg(image, rel):
    """A 256 px PNG embedded in an SVG, for resources Qt loads as .svg."""
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" '
        'xmlns:xlink="http://www.w3.org/1999/xlink" width="256" height="256" '
        'viewBox="0 0 256 256">\n'
        f'  <image width="256" height="256" xlink:href="{png_data_uri(image)}"/>\n'
        "</svg>\n"
    )
    (ROOT / rel).write_text(svg)
    print("wrote", rel)


def write_tray_icons(master):
    images = "frontend/forms/images"
    active(master).save(ROOT / f"{images}/tray_active.png", optimize=True)
    print(f"wrote {images}/tray_active.png")
    for name, icon in (
        ("obs_macos", mask(master)),
        ("obs_paused_macos", mask_badge(mask(master), "pause")),
        ("tray_active_macos", mask_badge(mask(master), "dot")),
    ):
        icon.save(ROOT / f"{images}/{name}.png", optimize=True)
        print(f"wrote {images}/{name}.png")
        write_png_svg(icon, f"{images}/{name}.svg")


def png_data_uri(image):
    buf = io.BytesIO()
    image.save(buf, format="PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def write_svg_wrapper(master, rel):
    """Linux scalable icon: there is no vector source, so embed a 512 px PNG."""
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" '
        'xmlns:xlink="http://www.w3.org/1999/xlink" width="512" height="512" '
        'viewBox="0 0 512 512">\n'
        f'  <image width="512" height="512" xlink:href="{png_data_uri(sized(master, 512))}"/>\n'
        "</svg>\n"
    )
    (ROOT / rel).write_text(svg)
    print("wrote", rel)


def replace_docs_mark(master, rel):
    """Swap the circular mark in the docs logo; keep the wordmark."""
    path = ROOT / rel
    svg = path.read_text()
    new, count = re.subn(
        r'(<image\b[^>]*?xlink:href=")[^"]*(")',
        lambda m: m.group(1) + png_data_uri(sized(master, 256)) + m.group(2),
        svg,
        count=1,
    )
    if count != 1:
        sys.exit(f"{rel}: no embedded mark image found")
    path.write_text(new)
    print("wrote", rel)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    master = cut_out(Image.open(sys.argv[1]))

    save_png(master, MASTER, "docs/images/obs-rust-logo.png")
    save_png(master, 256, "frontend/forms/images/obs.png")
    paused(master).save(ROOT / "frontend/forms/images/obs_paused.png", optimize=True)
    print("wrote frontend/forms/images/obs_paused.png")
    write_tray_icons(master)

    for size in (128, 256, 512):
        save_png(master, size, f"frontend/cmake/linux/icons/obs-logo-{size}.png")
    write_svg_wrapper(master, "frontend/cmake/linux/icons/obs-logo-scalable.svg")

    appiconset = "frontend/cmake/macos/Assets.xcassets/AppIcon.appiconset"
    for size in (16, 32, 128, 256, 512):
        save_png(master, size, f"{appiconset}/icon_{size}x{size}.png")
        save_png(master, size * 2, f"{appiconset}/icon_{size}x{size}@2x.png")
    master.save(ROOT / "cmake/macos/resources/AppIcon.icns", format="ICNS")
    print("wrote cmake/macos/resources/AppIcon.icns")

    windows_sizes = (16, 20, 24, 32, 40, 48, 64, 256)
    save_ico(master, windows_sizes, "frontend/cmake/windows/obs-studio.ico")
    save_ico(master, windows_sizes, "cmake/bundle/windows/obs-studio.ico")
    save_ico(master, (16, 32, 48), "docs/sphinx/favicon.ico")
    replace_docs_mark(master, "docs/sphinx/logo.svg")


if __name__ == "__main__":
    main()
