#!/usr/bin/env -S uv run --script
"""Run the test-only VST3 fixture against the binaries from a desktop package."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
from tempfile import TemporaryDirectory

import host_platform


# These are the two effect class IDs in the example bundle.
GAIN_CLASSES = ("57B71384C2EE4713B586F2FD7FDB08A1", "6BD4700D163F4219A265882F52B373E8")


def one_match(directory: Path, name: str) -> Path:
    matches = list(directory.rglob(name))
    if len(matches) != 1:
        raise RuntimeError(f"expected one {name} below {directory}, found {matches}")
    return matches[0]


def sample_timed_out_probe(process: subprocess.Popen) -> None:
    if not host_platform.is_macos() or process.poll() is not None:
        return
    print("::group::VST3 package probe timeout sample", flush=True)
    try:
        subprocess.run(
            ["sample", str(process.pid), "3", "-file", "/dev/stdout"],
            check=False,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        print(f"VST3 probe sample unavailable: {error}", flush=True)
    print("::endgroup::", flush=True)


def run_probe(app: Path, archive: Path, work: Path) -> None:
    with tarfile.open(archive) as bundle:
        bundle.extractall(work, filter="data")

    env = os.environ.copy()
    home = work / "home"
    if host_platform.is_windows():
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
    elif host_platform.is_macos():
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
        raise RuntimeError("the VST3 package probe requires Windows or macOS")

    stage = work / "bin"
    stage.mkdir()
    executable = stage / source.name
    shutil.move(source, executable)
    shutil.copy2(scanner, stage)
    vst3_dir.mkdir(parents=True)
    shutil.move(work / "OBSFixture.vst3", vst3_dir)
    config = work / "config"
    config.mkdir()
    command = [executable, module, resources, config, *GAIN_CLASSES, "--editor"]
    with subprocess.Popen(command, cwd=app.parent, env=env) as process:
        try:
            result = process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            try:
                sample_timed_out_probe(process)
            finally:
                process.kill()
                process.wait()
            raise
    if result:
        raise subprocess.CalledProcessError(result, command)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    args = parser.parse_args()
    with TemporaryDirectory(prefix="obs-vst3-package-") as work:
        run_probe(args.app.resolve(strict=True), args.archive, Path(work))


if __name__ == "__main__":
    main()
