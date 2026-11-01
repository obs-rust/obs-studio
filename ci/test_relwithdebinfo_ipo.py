"""RelWithDebInfo IPO is an opt-in switch, not a hard-coded OFF (#3193).

Usage:
    uv run python -m unittest ci/test_relwithdebinfo_ipo.py

The shipped builds are RelWithDebInfo, and compiler_common.cmake pinned
CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELWITHDEBINFO to OFF, so a packager had
no way to turn LTO on short of editing the file. OBS_RELWITHDEBINFO_IPO
(default OFF) is that switch; configured with it, build.ninja has -flto
(measured in #3193: 2,200 compile lines).
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMMON = ROOT / "cmake/common/compiler_common.cmake"


class RelWithDebInfoIpoTest(unittest.TestCase):
    def test_option_defaults_off(self):
        text = COMMON.read_text(encoding="utf-8")
        self.assertRegex(text, r'option\(OBS_RELWITHDEBINFO_IPO "[^"]+" OFF\)')

    def test_supported_ipo_follows_the_option(self):
        text = COMMON.read_text(encoding="utf-8")
        supported = text.split("if(HAS_INTERPROCEDURAL_OPTIMIZATION)\n", 2)[-1].split("else()", 1)[0]
        self.assertIn("set(CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELWITHDEBINFO ${OBS_RELWITHDEBINFO_IPO})", supported)
        self.assertIsNone(re.search(r"INTERPROCEDURAL_OPTIMIZATION_RELWITHDEBINFO OFF", supported))


if __name__ == "__main__":
    unittest.main()
