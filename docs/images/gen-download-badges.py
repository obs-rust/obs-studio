#!/usr/bin/env python3
"""Generate the platform download badges for the obs-rust README.

Dialect matches docs/images/discussions.svg and merge-queue.svg: flat two-tone
plate, rx=3, grey label side + one accent side, glyph as a path, 10% sheen.

Each plate is sized to its own content (as the house badges are) with one
uniform right inset, so the three form a set without ragged interior padding.
Every glyph is normalised into the same 20x20 optical box centred on the plate
midline, so the row reads as a single line of ink.
"""

from pathlib import Path

HEIGHT = 44
LEFT = 100          # label segment (symmetric label padding)
PAD = 14            # inner padding before the glyph
GLYPH = 20          # optical box for every platform mark
GAP = 8             # glyph -> word
RIGHT_PAD = 14
SAFETY = 10         # extra width so a wider system font cannot shrink the right inset
RADIUS = 3
SHEEN = ".10"

# 13px bold system-sans advance widths, measured from the rendered SVG.
ADVANCE = {
    "A": 9.4, "C": 9.4, "D": 10.4, "I": 4.0, "L": 8.3, "M": 12.4,
    "N": 10.4, "O": 10.4, "S": 9.4, "U": 10.4, "W": 13.6, "X": 9.4,
}
TRACKING = 0.8


def text_width(label: str) -> float:
    """Advance width of a platform word at 13px bold plus its tracking."""
    return sum(ADVANCE[c] for c in label) + TRACKING * len(label)


# --- glyphs, each drawn to fill a 20x20 box centred on (0,0) ---------------

WIN = '''    <g fill="#ffffff">
      <rect x="-10" y="-10" width="9.2" height="9.2" rx="0.8"/>
      <rect x="0.8" y="-10" width="9.2" height="9.2" rx="0.8"/>
      <rect x="-10" y="0.8" width="9.2" height="9.2" rx="0.8"/>
      <rect x="0.8" y="0.8" width="9.2" height="9.2" rx="0.8"/>
    </g>'''

MAC = '''    <g transform="scale(0.70) translate(0.8 -1.2)" fill="#ffffff">
      <path d="M2.4-10.6c1.1-1.3 1.8-3.1 1.6-4.9-1.5.1-3.3 1-4.2 2.2-.9 1.1-1.7 2.9-1.4 4.6 1.6.1 3.1-.8 4-1.9z"/>
      <path d="M4-1.4c-.1-2.8 2.3-4.2 2.4-4.3-1.3-1.9-3.4-2.2-4.1-2.3-1.7-.2-3.4 1-4.3 1-.9 0-2.3-1-3.8-.9-1.9.1-3.7 1.2-4.7 3-2 3.5-.5 8.7 1.4 11.5.9 1.3 2 2.8 3.5 2.8 1.4-.1 1.9-.9 3.6-.9s2.2.9 3.7.9c1.5 0 2.5-1.4 3.4-2.7 1.1-1.5 1.5-3 1.6-3.1-.1 0-3-1.2-3.1-4.3z"/>
    </g>'''

LNX = '''    <g transform="scale(0.80) translate(0 -1.2)">
      <ellipse cx="-9.6" cy="-0.6" rx="2.6" ry="6.2" fill="#ffffff"/>
      <ellipse cx="9.6" cy="-0.6" rx="2.6" ry="6.2" fill="#ffffff"/>
      <ellipse cx="0" cy="1.4" rx="9.4" ry="11.6" fill="#ffffff"/>
      <circle cx="-3.4" cy="-1.6" r="2.2" fill="#1b1f23"/>
      <circle cx="3.4" cy="-1.6" r="2.2" fill="#1b1f23"/>
      <path d="M-2.5 3.4h5.0l-2.5 3.5z" fill="#1b1f23"/>
    </g>'''
TPL = '''<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0f}" height="{h}" role="img" aria-label="{aria}">
  <title>{aria}</title>
  <defs>
    <filter id="sh{tint}" x="-10%" y="-10%" width="120%" height="120%">
      <feDropShadow dx="0" dy="1" stdDeviation="0" flood-color="#010101" flood-opacity=".30"/>
    </filter>
  </defs>
  <linearGradient id="g{tint}" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity="{sheen}"/>
    <stop offset="1" stop-opacity="{sheen}"/>
  </linearGradient>
  <clipPath id="c{tint}">
    <rect width="{w:.0f}" height="{h}" rx="{r}" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#c{tint})">
    <rect width="{left}" height="{h}" fill="#555555"/>
    <rect x="{left}" width="{w:.0f}" height="{h}" fill="{accent}"/>
    <rect width="{w:.0f}" height="{h}" fill="url(#g{tint})"/>
  </g>
  <g filter="url(#sh{tint})">
  <text x="{left_half}" y="{ty}" font-family="Verdana, 'DejaVu Sans', Geneva, sans-serif"
        font-size="11" font-weight="600" letter-spacing="1.0" fill="#ffffff"
        text-anchor="middle">DOWNLOAD</text>
  <g transform="translate({gx} {gy})">
{glyph}
  </g>
  <text x="{tx}" y="{ty}" font-family="Verdana, 'DejaVu Sans', Geneva, sans-serif"
        font-size="13" font-weight="700" letter-spacing="{tracking}" fill="#ffffff"
        text-anchor="end" text-rendering="geometricPrecision">{label}</text>
  </g>
</svg>
'''

SPECS = [
    ("download-windows.svg", "Download OBS-Studio-Rust for Windows", "WINDOWS", "#0067bf", "a", WIN),
    ("download-macos.svg", "Download OBS-Studio-Rust for macOS", "MACOS", "#8957e5", "b", MAC),
    ("download-linux.svg", "Download OBS-Studio-Rust for Linux", "LINUX", "#bc4c00", "c", LNX),
]


def main() -> None:
    out = Path("docs/images")
    out.mkdir(parents=True, exist_ok=True)
    for name, aria, label, accent, tint, glyph in SPECS:
        width = LEFT + PAD + GLYPH + GAP + text_width(label) + RIGHT_PAD + SAFETY
        svg = TPL.format(
            w=width, h=HEIGHT, aria=aria, tint=tint, accent=accent, sheen=SHEEN, r=RADIUS,
            left=LEFT, left_half=LEFT / 2,
            gx=LEFT + PAD + GLYPH / 2, gy=HEIGHT / 2,
            tx=width - RIGHT_PAD, ty=HEIGHT / 2 + 4.6, tracking=TRACKING,
            glyph=glyph, label=label,
        )
        (out / name).write_text(svg, encoding="utf-8")
        print(f"{name}: {width:.0f}x{HEIGHT}")


if __name__ == "__main__":
    main()
