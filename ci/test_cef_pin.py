#!/usr/bin/env -S uv run --script
"""Linux builds use CEF 8078 with proprietary codecs (obs-rust/obs-studio#1864).

    uv run python -m unittest ci.test_cef_pin

Browser sources on Linux need Chromium 156 or newer (#1092), and H.264/AAC
playback needs a CEF built with proprietary codecs, which the official CEF
builds are not. obs-rust builds one (obs-studio-libs recipes/cef/linux) and
publishes it to the obs-rust/cef mirror as release 8078.

Two Linux consumers read the pin: .github/scripts/utils.zsh/setup_ubuntu
(from CMakePresets.json, where `versions` overrides `version` per target, and
`releases` the release tag when a rebuild of the same branch is published
under a new one, such as 8078-r2, #2363) and the Flatpak manifest (its own
URL). They must agree, and neither may
fall back to 7871.

The macOS targets read the same two overrides through
`cmake/common/buildspec_common.cmake`, now that obs-rust publishes the 8078
macOS archives on the mirror (#2418). Windows stays on `version` until it has
a build host, so `cef_support.py` keeps failing on the Windows pins — that is
the remaining upgrade work, not a regression here.
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
    # Mirrors setup_ubuntu's jq: a per-target version, or a per-target
    # release tag when there is one, replaces the release segment of baseUrl.
    # The file name always carries the version.
    version = spec.get("versions", {}).get(target, spec["version"])
    release = spec.get("releases", {}).get(target, version)
    base = spec["baseUrl"].rsplit("/", 1)[0] + f"/{release}"
    arch = target.removeprefix("ubuntu-")
    return f"{base}/cef_binary_{version}_linux_{arch}.tar.xz"


# Targets obs-rust builds itself and publishes on the mirror. The rest keep
# `version` and its baseUrl tag.
BUILT_TARGETS = ("ubuntu-x86_64", "macos-arm64", "macos-x86_64")


def target_version(spec: dict, target: str) -> int:
    return int(spec.get("versions", {}).get(target, spec["version"]))


def target_release(spec: dict, target: str) -> str:
    version = target_version(spec, target)
    return spec.get("releases", {}).get(target, str(version))


def target_url(spec: dict, target: str) -> str:
    """The mirror URL `cmake/common/buildspec_common.cmake` downloads."""
    os_name, _, arch = target.partition("-")
    suffix = "zip" if os_name == "windows" else "tar.xz"
    release = target_release(spec, target)
    base = spec["baseUrl"].rsplit("/", 1)[0] + f"/{release}"
    return f"{base}/cef_binary_{target_version(spec, target)}_{os_name}_{arch}.{suffix}"


def flatpak_cef() -> dict:
    manifest = json.loads((ROOT / "build-aux/io.github.obs_rust.OBSStudioRust.json").read_text())
    for module in manifest["modules"]:
        if isinstance(module, dict) and module.get("name") == "cef":
            return module["sources"][0]
    raise AssertionError("Flatpak manifest has no cef module")


class CefPinTest(unittest.TestCase):
    def test_ubuntu_x86_64_is_at_the_floor(self):
        spec = cef_spec()
        version = spec.get("versions", {}).get("ubuntu-x86_64", spec["version"])
        self.assertGreaterEqual(int(version), FLOOR)
        release = spec.get("releases", {}).get("ubuntu-x86_64", version)
        self.assertTrue(release == version or release.startswith(f"{version}-r"), release)
        self.assertTrue(ubuntu_url(spec, "ubuntu-x86_64").startswith(f"{MIRROR}/{release}/"))

    def test_flatpak_matches_the_ubuntu_pin(self):
        spec = cef_spec()
        source = flatpak_cef()
        self.assertEqual(source["url"], ubuntu_url(spec, "ubuntu-x86_64"))
        self.assertEqual(source["sha256"], spec["hashes"]["ubuntu-x86_64"])

    def test_setup_ubuntu_reads_the_override(self):
        script = (ROOT / ".github/scripts/utils.zsh/setup_ubuntu").read_text()
        self.assertIn(".versions[$target]", script)
        self.assertIn(".releases[$target]", script)

    def test_other_targets_keep_their_release(self):
        # Only targets with an obs-rust build move; the rest keep `version`.
        spec = cef_spec()
        for target in [*spec.get("versions", {}), *spec.get("releases", {})]:
            self.assertIn(target, BUILT_TARGETS)
        self.assertTrue(spec["baseUrl"].endswith(f"/{spec['version']}"))
        for target in spec["hashes"]:
            if target not in BUILT_TARGETS:
                self.assertEqual(target_version(spec, target), int(spec["version"]))

    def test_every_built_target_is_at_the_floor(self):
        spec = cef_spec()
        for target in BUILT_TARGETS:
            with self.subTest(target=target):
                version = target_version(spec, target)
                release = target_release(spec, target)
                self.assertGreaterEqual(version, FLOOR)
                self.assertTrue(release == str(version) or release.startswith(f"{version}-r"), release)
                self.assertEqual(
                    target_url(spec, target),
                    f"{MIRROR}/{release}/cef_binary_{version}_{target.partition('-')[0]}"
                    f"_{target.rpartition('-')[2]}.{'zip' if target.startswith('windows') else 'tar.xz'}",
                )

    def test_macos_reads_the_platform_override(self):
        # macOS downloads through CMake, so the reader there has to honour the
        # same `versions` / `releases` overrides as setup_ubuntu.
        reader = (ROOT / "cmake/common/buildspec_common.cmake").read_text()
        self.assertIn("versions ${platform}", reader)
        self.assertIn("releases ${platform}", reader)


if __name__ == "__main__":
    unittest.main()
