#!/usr/bin/env -S uv run --script
"""Run the test-only VST3 fixture against the binaries from a desktop package."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
from tempfile import TemporaryDirectory


# These are the two effect class IDs in the example bundle.
GAIN_CLASSES = ("57B71384C2EE4713B586F2FD7FDB08A1", "6BD4700D163F4219A265882F52B373E8")


def one_match(directory: Path, name: str) -> Path:
    matches = list(directory.rglob(name))
    if len(matches) != 1:
        raise RuntimeError(f"expected one {name} below {directory}, found {matches}")
    return matches[0]


def run_probe(app: Path, archive: Path, work: Path) -> None:
    with tarfile.open(archive) as bundle:
        bundle.extractall(work, filter="data")

    env = os.environ.copy()
    home = work / "home"
    if sys.platform == "win32":
        env.update(
            LOCALAPPDATA=str(home),
            PROGRAMFILES=str(work / "program-files"),
            PATH=os.pathsep.join((str(app.parent), env["PATH"])),
            QT_QPA_PLATFORM="windows",
            QT_PLUGIN_PATH=str(app.parent),
        )
        source = work / "test_vst3_host.exe"
        scanner = app.parent / "obs-vst3-scanner.exe"
        module = app.parents[2] / "core/obs-vst3/obs-vst3.dll"
        resources = module.parent / "data"
        vst3_dir = home / "Programs/Common Files/VST3"
    elif sys.platform == "darwin":
        contents = app.parent.parent
        env.update(
            HOME=str(home),
            QT_QPA_PLATFORM="cocoa",
            QT_PLUGIN_PATH=str(one_match(contents, "libqcocoa.dylib").parents[1]),
        )
        source = work / "test_vst3_host"
        scanner = app.parent / "obs-vst3-scanner"
        plugin = one_match(contents, "obs-vst3.plugin")
        module = plugin / "Contents/MacOS/obs-vst3"
        resources = plugin / "Contents/Resources"
        vst3_dir = home / "Library/Audio/Plug-Ins/VST3"
        # Match the installed Mach-O @executable_path/../Frameworks runpaths.
        (work / "Frameworks").symlink_to(contents / "Frameworks", target_is_directory=True)
    else:
        raise RuntimeError(f"unsupported desktop package: {sys.platform}")

    stage = work / "bin"
    stage.mkdir()
    executable = stage / source.name
    shutil.move(source, executable)
    shutil.copy2(scanner, stage)
    vst3_dir.mkdir(parents=True)
    shutil.move(work / "OBSFixture.vst3", vst3_dir)
    config = work / "config"
    config.mkdir()
    subprocess.run(
        [executable, module, resources, config, *GAIN_CLASSES, "--editor"],
        cwd=app.parent,
        env=env,
        check=True,
        timeout=30,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    args = parser.parse_args()
    with TemporaryDirectory(prefix="obs-vst3-package-") as work:
        run_probe(args.app.resolve(strict=True), args.archive, Path(work))


if __name__ == "__main__":
    main()
