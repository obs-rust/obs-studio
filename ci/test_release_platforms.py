#!/usr/bin/env -S uv run --script
"""The daily beta builds a package for every platform we ship (#628, #629).

    uv run python -m unittest ci/test_release_platforms.py

release-rust.yaml's build matrix decides which packages a beta has. Intel
Macs got nothing: the matrix had only macos/arm64, although the release gate
already builds macOS x86_64. Windows arm64 also needs its own package and
Rust cross target on the x64 build runner.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "release-rust.yaml"
import release_decision

# (os, target) pairs every beta must be built for.
REQUIRED = {
    ("linux", "x86_64"),
    ("macos", "arm64"),
    ("macos", "x86_64"),
    ("windows", "x64"),
    ("windows", "arm64"),
}


def build_matrix(text: str) -> list[dict[str, str]]:
    """The `include:` entries of the build job's matrix."""
    m = re.search(r"^  build:\n(.*?)^  \S", text, re.S | re.M)
    assert m, "no build job in release-rust.yaml"
    block = re.search(r"^        include:\n((?:^          .*\n)+)", m.group(1), re.M)
    assert block, "no matrix include in the build job"
    entries: list[dict[str, str]] = []
    for line in block.group(1).splitlines():
        kv = re.match(r"^          (?:(- )|  )([a-z_]+): (\S+)", line)
        if not kv:
            continue
        if kv.group(1):
            entries.append({})
        entries[-1][kv.group(2)] = kv.group(3)
    return entries


class ReleasePlatformsTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text()
        self.matrix = build_matrix(self.text)

    def test_matrix_builds_every_shipped_platform(self):
        built = {(e["os"], e["target"]) for e in self.matrix}
        self.assertEqual(REQUIRED - built, set(), f"release-rust.yaml builds {sorted(built)}")

    def test_artifact_names_are_unique_per_target(self):
        # Two macOS legs uploading obs-studio-rust-macos would collide.
        m = re.search(r"name: (obs-studio-rust-.+)\n\s+path: pkg/", self.text)
        self.assertIsNotNone(m, "no package upload in the build job")
        self.assertIn("matrix.target", m.group(1))

    def test_collect_finds_the_intel_dmg(self):
        # package-obs names the x86_64 DMG *-macos-intel.dmg, not -apple.
        self.assertIn("kind=intel", self.text, "Collect and verify looks only for -apple.dmg")

    def test_collect_selects_each_windows_architecture(self):
        # Execute the workflow's selector with both packages present. An
        # arm64 leg must never silently collect the x64 zip (#629).
        case = re.search(r'case "\$OS" in\n(.*?)\n\s+esac', self.text, re.S)
        self.assertIsNotNone(case, "no package selector in the build job")
        script = case.group(0) + '\nprintf "%s\\n" "${found[@]}"\n'
        with tempfile.TemporaryDirectory() as clean:
            for target in ("x64", "arm64"):
                directory = Path(clean) / f"build_{target}"
                directory.mkdir()
                (directory / f"obs-studio-33.0.0-windows-{target}.zip").touch()
            for target in ("x64", "arm64"):
                with self.subTest(target=target):
                    result = subprocess.run(
                        ["bash", "-euc", script],
                        cwd=clean,
                        env={**os.environ, "OS": "windows", "TARGET": target},
                        check=True,
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(
                        result.stdout.strip(),
                        f"build_{target}/obs-studio-33.0.0-windows-{target}.zip",
                    )

    def test_release_refuses_missing_windows_arm64_zip(self):
        version = "0.1.0-beta.20261013"
        arm64 = f"OBS-Studio-Rust-{version}-windows-arm64.zip"
        present = [name for name in release_decision.required_assets(version) if name != arm64]
        self.assertEqual(release_decision.missing_assets(version, present), [arm64])

    def test_release_refuses_missing_flatpak(self):
        # A beta ships a .flatpak next to the .deb and the AppImage (#2400).
        version = "0.1.0-beta.20261026"
        flatpak = f"OBS-Studio-Rust-{version}-linux-x86_64.flatpak"
        self.assertIn(flatpak, release_decision.required_assets(version))
        present = [name for name in release_decision.required_assets(version) if name != flatpak]
        self.assertEqual(release_decision.missing_assets(version, present), [flatpak])

    def test_release_ships_the_bundle_the_gate_ran(self):
        # The gate's Flatpak Saved Profile job installs the bundle with
        # `flatpak install --bundle` and runs it; the release publishes that file.
        gate = re.search(r"^  gate:\n(.*?)^  release:", self.text, re.M | re.S)
        self.assertIsNotNone(gate)
        self.assertIn("flatpak-runtime-test: true", gate.group(1))
        release = self.text.split("\n  release:\n", 1)[1]
        self.assertIn("name: flatpak-bundle", release)
        self.assertIn('flatpak/obs-studio-rust-x86_64.flatpak "dist/OBS-Studio-Rust-$VERSION-linux-x86_64.flatpak"', release)

    def test_windows_arm64_rust_target_is_installed_before_build(self):
        steps = self.text.split("      - name: ")
        provision = next(
            (step for step in steps if "run: soldr rustup target add aarch64-pc-windows-msvc" in step), None
        )
        self.assertIsNotNone(provision, "the x64 runner needs Rust's Windows arm64 standard library")
        self.assertIn("matrix.os == 'windows'", provision)
        self.assertIn("matrix.target == 'arm64'", provision)
        build_index = next(i for i, step in enumerate(steps) if step.startswith("Build OBS-Studio-Rust"))
        self.assertLess(steps.index(provision), build_index)

    def test_release_appimage_call_explicitly_enables_rust(self):
        call = re.search(r"^  appimage:\n(.*?)^  gate:", self.text, re.M | re.S)
        self.assertIsNotNone(call)
        self.assertTrue("rust_libobs: true" in call.group(1), "manual release calls can build a C-only AppImage")

    def test_called_appimage_accepts_the_rust_build_option(self):
        text = (ROOT / ".github/workflows/appimage.yaml").read_text()
        call = re.search(r"^  workflow_call:\n(.*?)^  workflow_dispatch:", text, re.M | re.S)
        self.assertIsNotNone(call)
        self.assertRegex(call.group(1), r"rust_libobs:\n\s+description:.*\n\s+type: boolean")

    def test_release_requires_every_built_package(self):
        # The release refuses to publish without these (#622); a leg the
        # matrix builds must be on the list, or it can silently go missing.
        required = release_decision.required_assets("1.0")
        for os_, target in sorted(REQUIRED - {("linux", "x86_64")}):
            with self.subTest(os=os_, target=target):
                self.assertTrue(
                    any(f"-{os_}-{target}." in a for a in required),
                    f"{os_}-{target} missing from {required}",
                )

    def test_continuous_keeps_its_appimage(self):
        # The continuous step uploads the canonical AppImage after the beta.
        # Moving it into dist/ for the beta previously left
        # only the .zsync, so run 38500530155 published the beta and then
        # failed with "no matches found for OBS_Studio-*-x86_64.AppImage".
        self.assertIsNone(
            re.search(r"^\s*mv\s+appimage/OBS-Studio-Rust-", self.text, re.M),
            "the beta step moves the AppImage the continuous step still needs; copy it",
        )
        self.assertIn("OBS-Studio-Rust-*-linux-x86_64.AppImage OBS-Studio-Rust-*-linux-x86_64.AppImage.zsync", self.text)


if __name__ == "__main__":
    unittest.main()
