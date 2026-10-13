"""Regenerate text-aa.ttf with FontTools; all outlines are synthetic geometry.

The checked-in font keeps test_text_aa_file independent of installed fonts.
FontTools is needed only to regenerate it, not to build or run the test.
"""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

rectangle = [(50, 0), (550, 0), (550, 700), (50, 700)]
accent = [(200, 760), (350, 760), (500, 900), (350, 900)]
outlines = {".notdef": [rectangle], "space": [], "A": [rectangle], "eacute": [rectangle, accent]}
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
font.setupCharacterMap({32: "space", 65: "A", 233: "eacute"})
font.setupGlyf(glyphs)
font.setupHorizontalMetrics({name: (600, 50) for name in outlines})
font.setupHorizontalHeader(ascent=950, descent=-50)
font.setupNameTable({
    "familyName": "OBS AA Fixture",
    "styleName": "Regular",
    "uniqueFontIdentifier": "obs-cmocka-aa-fixture",
    "fullName": "OBS AA Fixture Regular",
    "psName": "OBSAAFixture-Regular",
})
font.setupOS2(sTypoAscender=950, sTypoDescender=-50, usWinAscent=950, usWinDescent=50)
font.setupPost()
font.setupMaxp()
font.font.recalcTimestamp = False
font.font["head"].created = font.font["head"].modified = 2082844800
font.save(Path(__file__).with_suffix(".ttf"))
