#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["fonttools==4.60.1"]
# ///
"""Regenerate text-collection.ttc with FontTools; all outlines are synthetic geometry.

A font collection with two faces of one family, Regular (face 0) and Bold
(face 1), for test_text_freetype2_face_index. FontTools is needed only to
regenerate it, not to build or run the test: `uv run
test/cmocka/data/text-collection.py` rewrites the file byte for byte. The
outlines are plain rectangles written here, so the font carries no third-party
glyph data and is under this repository's licence.
"""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection


def face(style, weight, stem):
    rectangle = [(50, 0), (50 + stem, 0), (50 + stem, 700), (50, 700)]
    outlines = {".notdef": [rectangle], "space": [], "A": [rectangle]}
    glyphs = {}
    for name, contours in outlines.items():
        pen = TTGlyphPen(None)
        for contour in contours:
            pen.moveTo(contour[0])
            for point in contour[1:]:
                pen.lineTo(point)
            pen.closePath()
        glyphs[name] = pen.glyph()

    font = FontBuilder(1000, isTTF=True)
    font.setupGlyphOrder(list(outlines))
    font.setupCharacterMap({32: "space", 65: "A"})
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics({name: (600, 50) for name in outlines})
    font.setupHorizontalHeader(ascent=950, descent=-50)
    font.setupNameTable({
        "familyName": "OBS Collection Fixture",
        "styleName": style,
        "uniqueFontIdentifier": f"obs-cmocka-collection-fixture-{style.lower()}",
        "fullName": f"OBS Collection Fixture {style}",
        "psName": f"OBSCollectionFixture-{style}",
    })
    font.setupOS2(sTypoAscender=950, sTypoDescender=-50, usWinAscent=950, usWinDescent=50,
                  usWeightClass=weight, fsSelection=0x20 if weight >= 700 else 0x40)
    font.setupPost()
    font.setupMaxp()
    font.font.recalcTimestamp = False
    font.font["head"].created = font.font["head"].modified = 2082844800
    font.font["head"].macStyle = 1 if weight >= 700 else 0
    return font.font


collection = TTCollection()
collection.fonts = [face("Regular", 400, 200), face("Bold", 700, 400)]
collection.save(Path(__file__).with_suffix(".ttc"))
