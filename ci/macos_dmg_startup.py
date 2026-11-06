#!/usr/bin/env -S uv run --script
"""Install and verify the DMG, then run the unchanged four startup modes."""

import os
import platform
import plistlib
import re
import subprocess
from pathlib import Path


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def main():
    target, version = os.environ["TARGET"], os.environ["VERSION"]
    if platform.machine() != target:
        raise RuntimeError("runtime runner is not the package architecture")
    root = Path(os.environ["RUNNER_TEMP"]) / f"obs-macos-{target}"
    mount = Path(os.environ["RUNNER_TEMP"]) / f"obs-macos-mount-{target}"
    root.mkdir()
    mount.mkdir()
    dmg = Path("macos-package") / f"OBS-Studio-Rust-{version}-macos-{target}.dmg"
    run("hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", mount, dmg)
    try:
        app = root / "OBS-Studio-Rust.app"
        run("ditto", mount / app.name, app)
        run("lipo", app / "Contents/MacOS/OBS-Studio-Rust", "-verify_arch", target)
        run("codesign", "--verify", "--deep", "--strict", app)
        run("codesign", "-dvv", app)
        run("uv", "run", "--no-project", "ci/app_identity_check.py", "bundle", app)
        with (app / "Contents/Info.plist").open("rb") as source:
            plist = plistlib.load(source)
        if plist["CFBundleShortVersionString"] != version.split("-")[0]:
            raise RuntimeError("bundle short version mismatch")
        build = plist["CFBundleVersion"]
        if "-beta." in version:
            stamp = version.rsplit(".", 1)[1]
            expected = f"{stamp[:4]}.{int(stamp[4:6])}.{int(stamp[6:8])}"
            valid = build == expected
        else:
            valid = re.fullmatch(r"20[0-9]{2}\.[1-9][0-9]?\.[1-9][0-9]?", build)
        if not valid or plist["OBSStudioRustVersion"] != version:
            raise RuntimeError("bundle build/product version mismatch")
        run("uv", "sync", "--script", "test/packaged-startup.py", "--locked")
        for mode in ("fresh", "migrate-default", "migrate-custom", "existing"):
            run(
                "uv",
                "run",
                "--no-project",
                "--locked",
                "test/packaged-startup.py",
                "--app",
                app,
                "--config-dir",
                Path.home() / "Library/Application Support/obs-studio-rust",
                "--platform",
                "macos",
                "--config-mode",
                mode,
            )
    finally:
        subprocess.run(["hdiutil", "detach", str(mount), "-force"], check=False)


if __name__ == "__main__":
    main()
