#!/usr/bin/env -S uv run --script
"""The browser source's helper apps follow the app identity on macOS (#2386).

    uv run --no-project ci/test_macos_browser_helpers.py

Chromium finds its helpers by the main executable's name: "<app> Helper",
"<app> Helper (GPU)", "(Plugin)" and "(Renderer)" in Contents/Frameworks.
obs-browser named them "OBS Helper" while the Rust brand's app is
"OBS-Studio-Rust", so the GPU process failed to launch (error_code=1003) and
adding a browser source crashed OBS. The helpers' bundle IDs follow the app's
bundle ID the same way ("<bundle id>.helper", ".helper.gpu", ...).
"""

from __future__ import annotations

import importlib.util
import plistlib
import re
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BROWSER_CMAKE = ROOT / "plugins/obs-browser/cmake/os-macos.cmake"
# Jobs that check out no submodules (the Format job) skip the source checks;
# the bundle checks below still run, and the release workflow runs
# app_identity_check.py on every built macOS app.
NEEDS_BROWSER = unittest.skipUnless(BROWSER_CMAKE.is_file(), "plugins/obs-browser is not checked out")
FLAVORS = (("", ""), (" (GPU)", ".gpu"), (" (Plugin)", ".plugin"), (" (Renderer)", ".renderer"))


def identities() -> dict[str, dict[str, str]]:
    """OBS_MACOS_BUNDLE_NAME and _ID per brand, from cmake/common/app-identity.cmake."""
    text = (ROOT / "cmake/common/app-identity.cmake").read_text()
    head, rust = text.split("if(OBS_BRAND_RUST)", 1)
    rust = rust.split("endif()", 1)[0]

    def values(block: str) -> dict[str, str]:
        return dict(re.findall(r"set\((OBS_MACOS_BUNDLE_(?:NAME|ID))\s+(\S+)\)", block))

    upstream = values(head)
    return {"upstream": upstream, "rust": {**upstream, **values(rust)}}


def browser_helpers(identity: dict[str, str]) -> list[tuple[str, str]]:
    """(name, bundle id) of each helper that obs-browser's macOS CMake builds."""
    text = BROWSER_CMAKE.read_text()
    name = re.search(r'set\(helper_output_name\s+"([^"]+)"\)', text).group(1)
    bundle = re.search(r"XCODE_ATTRIBUTE_PRODUCT_BUNDLE_IDENTIFIER\s+(\S+)", text).group(1)
    for var, value in identity.items():
        name = name.replace(f"${{{var}}}", value)
        bundle = bundle.replace(f"${{{var}}}", value)
    return [(name + suffix, bundle.replace("${helper_plist}", plist)) for suffix, plist in FLAVORS]


def load_identity_check():
    spec = importlib.util.spec_from_file_location("app_identity_check", ROOT / "ci/app_identity_check.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_bundle(root: Path, helper_name: str, helper_id: str) -> Path:
    """A minimal OBS-Studio-Rust.app that check_bundle() accepts, with browser helpers."""
    check = load_identity_check()
    app = root / "OBS-Studio-Rust.app"
    (app / "Contents/MacOS").mkdir(parents=True)
    (app / "Contents/MacOS/OBS-Studio-Rust").write_bytes(b"")
    (app / "Contents/Frameworks/Sparkle.framework").mkdir(parents=True)
    with (app / "Contents/Info.plist").open("wb") as stream:
        plistlib.dump(
            {
                "CFBundleIdentifier": check.APP_ID,
                "CFBundleExecutable": "OBS-Studio-Rust",
                "CFBundleDisplayName": "OBS-Studio-Rust",
                "SUFeedURL": f"{check.UPDATE_FEED_PREFIX}updates_arm64_v2.xml",
                "SUPublicEDKey": check.SPARKLE_PUBLIC_KEY,
            },
            stream,
        )
    for suffix, plist in FLAVORS:
        helper = app / f"Contents/Frameworks/{helper_name}{suffix}.app/Contents"
        (helper / "MacOS").mkdir(parents=True)
        (helper / f"MacOS/{helper_name}{suffix}").write_bytes(b"")
        with (helper / "Info.plist").open("wb") as stream:
            plistlib.dump({"CFBundleExecutable": f"{helper_name}{suffix}", "CFBundleIdentifier": helper_id + plist}, stream)
    return app


class MacosBrowserHelpersTest(unittest.TestCase):
    @NEEDS_BROWSER
    def test_helpers_follow_the_app_identity(self):
        for brand, identity in identities().items():
            with self.subTest(brand=brand):
                expected = [
                    (f"{identity['OBS_MACOS_BUNDLE_NAME']} Helper{suffix}", f"{identity['OBS_MACOS_BUNDLE_ID']}.helper{plist}")
                    for suffix, plist in FLAVORS
                ]
                self.assertEqual(browser_helpers(identity), expected)

    @NEEDS_BROWSER
    def test_upstream_names_are_unchanged(self):
        self.assertEqual(browser_helpers(identities()["upstream"])[0], ("OBS Helper", "com.obsproject.obs-studio.helper"))

    def test_bundle_check_rejects_upstream_helper_names(self):
        check = load_identity_check()
        with tempfile.TemporaryDirectory() as tmp:
            app = write_bundle(Path(tmp), "OBS Helper", "com.obsproject.obs-studio.helper")
            with self.assertRaisesRegex(RuntimeError, "Helper"):
                check.check_bundle(app)

    def test_bundle_check_accepts_branded_helpers(self):
        check = load_identity_check()
        with tempfile.TemporaryDirectory() as tmp:
            app = write_bundle(Path(tmp), "OBS-Studio-Rust Helper", f"{check.APP_ID}.helper")
            check.check_bundle(app)


if __name__ == "__main__":
    unittest.main()
