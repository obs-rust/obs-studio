#!/usr/bin/env -S uv run --script
"""Linux builds use CEF 8078 with proprietary codecs (obs-rust/obs-studio#1864).

    uv run python -m unittest ci.test_cef_pin

Browser sources on Linux need Chromium 156 or newer (#1092), and H.264/AAC
playback needs a CEF built with proprietary codecs, which the official CEF
builds are not. obs-rust builds one (obs-studio-libs recipes/cef/linux) and
publishes it to the obs-rust/cef mirror as release 8078.

Two Linux consumers read the pin: .github/scripts/utils.zsh/setup_ubuntu
(from CMakePresets.json, where `versions` overrides `version` per target)
and the Flatpak manifest (its own URL). They must agree, and neither may
fall back to 7871. macOS and Windows stay on `version` until they have a
build host.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FLOOR = 8078
MIRROR = "https://github.com/obs-rust/cef/releases/download"


def cef_spec() -> dict:
    presets = json.loads((ROOT / "CMakePresets.json").read_text())
    for preset in presets["configurePresets"]:
        if preset["name"] == "dependencies":
            return preset["vendor"]["obsproject.com/obs-studio"]["dependencies"]["cef"]
    raise AssertionError("CMakePresets.json has no dependencies preset")


def ubuntu_url(spec: dict, target: str) -> str:
    # Mirrors setup_ubuntu's jq: a per-target version replaces the release
    # segment of baseUrl.
    version = spec.get("versions", {}).get(target, spec["version"])
    base = spec["baseUrl"].rsplit("/", 1)[0] + f"/{version}"
    arch = target.removeprefix("ubuntu-")
    return f"{base}/cef_binary_{version}_linux_{arch}.tar.xz"


def flatpak_cef() -> dict:
    manifest = json.loads((ROOT / "build-aux/com.obsproject.Studio.json").read_text())
    for module in manifest["modules"]:
        if isinstance(module, dict) and module.get("name") == "cef":
            return module["sources"][0]
    raise AssertionError("Flatpak manifest has no cef module")


class CefPinTest(unittest.TestCase):
    def test_ubuntu_x86_64_is_at_the_floor(self):
        spec = cef_spec()
        version = spec.get("versions", {}).get("ubuntu-x86_64", spec["version"])
        self.assertGreaterEqual(int(version), FLOOR)
        self.assertTrue(ubuntu_url(spec, "ubuntu-x86_64").startswith(f"{MIRROR}/{version}/"))

    def test_flatpak_matches_the_ubuntu_pin(self):
        spec = cef_spec()
        source = flatpak_cef()
        self.assertEqual(source["url"], ubuntu_url(spec, "ubuntu-x86_64"))
        self.assertEqual(source["sha256"], spec["hashes"]["ubuntu-x86_64"])

    def test_setup_ubuntu_reads_the_override(self):
        script = (ROOT / ".github/scripts/utils.zsh/setup_ubuntu").read_text()
        self.assertIn(".versions[$target]", script)

    def test_other_targets_keep_their_release(self):
        # Only targets with an obs-rust build move; the rest keep `version`.
        spec = cef_spec()
        for target in spec.get("versions", {}):
            self.assertEqual(target, "ubuntu-x86_64")
        self.assertTrue(spec["baseUrl"].endswith(f"/{spec['version']}"))


if __name__ == "__main__":
    unittest.main()
