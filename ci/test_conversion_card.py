#!/usr/bin/env -S uv run --script
"""The conversion progress card matches the leaderboard and sits near the top
of the README.

    uv run python -m unittest ci/test_conversion_card.py
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "README.md"
WORKFLOW = ROOT / ".github" / "workflows" / "leaderboard.yml"
SITE = ROOT / "tools" / "leaderboard" / "site"
RENDER = ROOT / "tools" / "leaderboard" / "render.py"
MODULE = ROOT / "ci" / "leaderboard.py"


def headings(text: str) -> list[str]:
    return re.findall(r"^# (.+)$", text, re.M)


class ConversionCard(unittest.TestCase):
    def test_page_uses_the_leaderboard_style(self):
        page = (SITE / "conversion.html").read_text()
        board = (SITE / "index.html").read_text()
        # Same palette, fonts and plate frame as the leaderboard.
        for token in ("--rust:#e8631a", "Big+Shoulders+Stencil+Display", "JetBrains+Mono", 'class="haz"', "logo.png"):
            self.assertIn(token, board)
            self.assertIn(token, page)
        self.assertIn("conversion.json", page)
        self.assertIn("dataset.ready", page)

    def test_render_takes_a_page_and_an_output(self):
        # The two renders moved into ci/leaderboard.py when the step became one
        # line (GEN-005, #2812), so the page/output pair is pinned there and the
        # workflow only has to call the module.
        render = RENDER.read_text()
        module = MODULE.read_text()
        self.assertIn("ci/leaderboard.py render", WORKFLOW.read_text())
        self.assertIn("conversion.html", module)
        self.assertIn("conversion.jpg", module)
        self.assertRegex(render, r"sys\.argv\[2\]")

    def test_measure_runs_before_the_render(self):
        wf = WORKFLOW.read_text()
        self.assertLess(wf.index("tools/conversion/measure.py"), wf.index("ci/leaderboard.py render"))

    def test_measure_runs_before_both_renders(self):
        wf = WORKFLOW.read_text()
        module = MODULE.read_text()
        self.assertLess(wf.index("tools/conversion/measure.py"), wf.index("ci/leaderboard.py render"))
        full = module.index("conversion.html")
        minimal = module.index("conversion-minimal.html")
        self.assertLess(full, minimal)

    def test_minimal_card_headline_is_libobs(self):
        page = (SITE / "conversion-minimal.html").read_text()
        for token in ("--rust:#e8631a", "Big+Shoulders+Stencil+Display", "JetBrains+Mono",
                      "conversion.json", "dataset.ready", "d.libobs.all.percent"):
            self.assertIn(token, page)
        # The headline is the libobs share: libobs is what has to be ported,
        # while the rest of the app is allowed to stay C++ in the hybrid
        # build. The whole-app percentage is not the number to lead with.
        self.assertIn("of libobs is Rust", page)
        self.assertNotIn("of the whole app is Rust", page)
        self.assertNotIn("app.percent", page)
        # The dense card lives on conversion.html; keep it out of this one.
        for label in ("UTIL", "MEDIA-IO", "GRAPHICS", "LINUX", "MACOS", "WINDOWS", "areas", "hosts"):
            self.assertNotIn(label.lower(), page.lower())

    def test_readme_shows_the_card_near_the_top(self):
        text = README.read_text()
        heads = headings(text)
        self.assertIn("Conversion progress", heads)
        self.assertLess(heads.index("Conversion progress"), heads.index("Features"))
        self.assertIn("leaderboard/conversion-minimal.jpg", text)
        # The image stays a link to the full breakdown.
        self.assertIn("](https://obs-rust.github.io/obs-studio/conversion.html)", text)
        self.assertIn("tools/conversion/measure.py", text)
        # Right after the "Got an idea?" line, before the Discord tip.
        idea = text.index("Got an idea? [Start a conversation]")
        self.assertLess(idea, text.index("# Conversion progress"))
        self.assertLess(text.index("# Conversion progress"), text.index("> [!TIP]"))


if __name__ == "__main__":
    unittest.main()
