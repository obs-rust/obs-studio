#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Build the OBS-Studio-Rust AppImage in the glibc 2.28 image
(build-aux/appimage/manylinux/Dockerfile, #926), whose /opt/obs-deps holds
every library it bundles.

    uv run build-aux/appimage/build.py                      # every step
    uv run build-aux/appimage/build.py --steps version,configure --dry-run

These are the build steps of .github/workflows/appimage.yaml, which calls this
script one step at a time. Each step runs the shell script it ran inline in
the workflow, unchanged, with `bash --noprofile --norc -eo pipefail` (the
Actions default shell) from the repository root. Workflow inputs are flags,
the step environment is set here, and `sudo` is a pass-through when running
as root without sudo (a container). The first failing step stops the build.

The AppImage is written to the repository root (where the workflow expects
it), and moved into --output-dir when one is given.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# A tagged release (not the rolling "continuous" build), verified by checksum. To
# update: change both, e.g. sha256sum of .../releases/download/<tag>/appimagetool-x86_64.AppImage
APPIMAGETOOL_VERSION = "1.9.1"
APPIMAGETOOL_SHA256 = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"


@dataclass(frozen=True)
class Step:
    name: str
    title: str
    script: str
    env: dict[str, str] = field(default_factory=dict)


VERSION = r"""
# Same tag filter as cmake/common/versionconfig.cmake: this repository also carries
# non-OBS tags (obs-studio-rust-v...), which must not become the OBS version.
# In a fork or shallow clone without OBS release tags `git describe` yields a bare
# commit hash, which CMake rejects as a project VERSION, so use a dev version then.
# It keeps the libobs API version: plugins compare the OBS version against the one
# they need, and CMake rejects an override older than the libobs API major (#1087).
if desc="$(git describe --tags --match '[0-9]*.[0-9]*.[0-9]*' 2>/dev/null)" && [[ "$desc" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "OBS_VERSION=$desc" >> "$GITHUB_ENV"
else
  api="$(sed -n 's/^#define LIBOBS_API_\(MAJOR\|MINOR\|PATCH\)_VER \([0-9]*\)$/\2/p' libobs/obs-config.h | paste -sd.)"
  echo "OBS_VERSION=${api}-g${GITHUB_SHA::9}" >> "$GITHUB_ENV"
fi
"""

CONFIGURE = r"""
echo "rust=${RUST_LIBOBS^^}" >> "$GITHUB_OUTPUT" # the tests check the build really has it
cmake --preset ubuntu-ci \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=OFF \
  -DCMAKE_INSTALL_PREFIX=/usr \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DOBS_VERSION_OVERRIDE="${OBS_VERSION}" \
  -DENABLE_RELOCATABLE=ON \
  -DENABLE_RUST_LIBOBS="${RUST_LIBOBS^^}" \
  -DENABLE_QSV11="${QSV^^}" \
  -DENABLE_BROWSER=OFF -DENABLE_VLC=OFF -DENABLE_WEBRTC=ON -DENABLE_AJA=OFF \
  -DENABLE_SCRIPTING_PYTHON=OFF
"""

BUILD = r"""
# Journal each compilation's hit or miss and the miss reason; the workflow
# summarizes it (#2182). A failed session start only loses the journal.
if ZCCACHE_SESSION_ID="$(zccache session-start --cwd "$PWD" --stats --journal "${RUNNER_TEMP:-/tmp}/appimage-zccache.jsonl" |
  sed -n 's/.*"session_id": *"\([^"]*\)".*/\1/p')" && [ -n "$ZCCACHE_SESSION_ID" ]; then
  export ZCCACHE_SESSION_ID
fi
# rcc writes each resource file's mtime into qrc_*.cpp, and a checkout gives
# every file a new one, so the qrc objects missed the cache on every build of
# the same commit (#2182). Not SOURCE_DATE_EPOCH: zccache keys on it.
epoch="$(git log -1 --format=%ct)"
{ git ls-files -z; git submodule foreach --quiet --recursive 'git ls-files -z | sed -z "s|^|$sm_path/|"'; } |
  xargs -0 touch -h -c -d "@$epoch"
cmake --build build_ubuntu
if [ -n "${ZCCACHE_SESSION_ID:-}" ]; then zccache session-end "$ZCCACHE_SESSION_ID" || true; fi
zccache status || true
zccache stop || true # flush the cache index before the workflow saves it
"""

APPDIR = r"""
DESTDIR="$PWD/AppDir" cmake --install build_ubuntu
uv run --no-project build-aux/appimage/assemble_appdir.py AppDir "${AVUTIL_LIB}"
"""

APPIMAGE = r"""
curl -fsSLo appimagetool "${APPIMAGETOOL_URL}"
echo "${APPIMAGETOOL_SHA256}  appimagetool" | sha256sum -c -
chmod +x appimagetool
# Runners have no /dev/fuse: extract appimagetool instead of running it.
./appimagetool --appimage-extract >/dev/null
# A build published by the release job carries update information that
# follows the certified ("latest") release, as the Windows and macOS updaters
# do (#1881): AppImageUpdate and friends fetch the next certified build as a
# delta against this one (.zsync), never an uncertified daily.
update=()
if [ "${PUBLISH_CONTINUOUS}" = true ]; then
  update=(-u "gh-releases-zsync|${GITHUB_REPOSITORY%/*}|${GITHUB_REPOSITORY#*/}|latest|OBS-Studio-Rust-*-linux-x86_64.AppImage.zsync")
fi
version="$(AppDir/AppRun --version)"
asset="$(uv run --no-project build-aux/appimage/asset_metadata.py name --version "$version")"
ARCH=x86_64 squashfs-root/AppRun --no-appstream "${update[@]}" AppDir "$asset"
ls -lh OBS-Studio-*
"""

STEP_NAMES = ("version", "configure", "build", "appdir", "appimage")
# Left behind by an earlier local run; a CI checkout never has them.
STALE = {"appdir": ("AppDir",), "appimage": ("appimagetool", "squashfs-root")}


def bool_flag(value: str) -> str:
    value = value.strip().lower()
    if value not in {"true", "false"}:
        raise argparse.ArgumentTypeError(f"expected true or false, got {value!r}")
    return value


def default_sha() -> str:
    if os.environ.get("GITHUB_SHA"):
        return os.environ["GITHUB_SHA"]
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else "000000000"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--steps", default=",".join(STEP_NAMES),
                   help=f"comma-separated subset of {','.join(STEP_NAMES)} (run in that order)")
    # The only base since the Ubuntu 26.04 one was retired (#1999); kept so the
    # workflow's `--base manylinux` stays valid.
    p.add_argument("--base", choices=("manylinux",), default="manylinux",
                   help="the glibc 2.28 image build-aux/appimage/manylinux/Dockerfile (#926)")
    p.add_argument("--rust-libobs", type=bool_flag, default="true", help="ENABLE_RUST_LIBOBS (workflow input rust_libobs)")
    p.add_argument("--qsv", type=bool_flag, default="false", help="ENABLE_QSV11 (workflow input qsv)")
    p.add_argument("--rust-version", default="", help="OBS_RUST_VERSION to embed (workflow input rust_version)")
    p.add_argument("--publish-continuous", type=bool_flag, default="false",
                   help="embed continuous-release update information (workflow input publish_continuous)")
    p.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", "obs-rust/obs-studio"),
                   help="owner/name for the update information (default: $GITHUB_REPOSITORY)")
    p.add_argument("--sha", default=None, help="commit for the dev version fallback (default: $GITHUB_SHA or HEAD)")
    p.add_argument("--output-dir", type=Path, default=None, help="move the AppImage (and .zsync) here at the end")
    p.add_argument("--dry-run", action="store_true", help="print the planned steps and their scripts; run nothing")
    args = p.parse_args(argv)
    steps = [s.strip() for s in args.steps.split(",") if s.strip()]
    unknown = sorted(set(steps) - set(STEP_NAMES))
    if unknown:
        p.error(f"unknown step(s): {', '.join(unknown)}")
    args.steps = [s for s in STEP_NAMES if s in steps]
    if args.sha is None:
        args.sha = default_sha()
    return args


def plan(args: argparse.Namespace) -> list[Step]:
    """The steps to run, in workflow order, with each one's environment."""
    every = {
        "version": Step("version", "Determine version", VERSION, {"GITHUB_SHA": args.sha}),
        "configure": Step("configure", "Configure", CONFIGURE, {
            "RUST_LIBOBS": args.rust_libobs, "QSV": args.qsv, "OBS_RUST_VERSION": args.rust_version,
        }),
        "build": Step("build", "Build", BUILD),
        "appdir": Step("appdir", "Assemble AppDir", APPDIR, {
            "AVUTIL_LIB": "/opt/obs-deps/ffmpeg/lib/libavutil.so.60",
        }),
        "appimage": Step("appimage", "Create AppImage", APPIMAGE, {
            "APPIMAGETOOL_VERSION": APPIMAGETOOL_VERSION, "APPIMAGETOOL_SHA256": APPIMAGETOOL_SHA256,
            # The glibc 2.28 image has it from obs-rust/obs-studio-libs (the same bytes, so the
            # same sha256 check applies).
            "APPIMAGETOOL_URL": "file:///opt/obs-deps/appimagetool/appimagetool-x86_64.AppImage",
            "PUBLISH_CONTINUOUS": args.publish_continuous, "GITHUB_REPOSITORY": args.repository,
        }),
    }
    return [every[name] for name in args.steps]


# Root in a container usually has no sudo; the scripts keep their `sudo -E`.
SUDO_SHIM = 'sudo() { [ "$1" != -E ] || shift; "$@"; }\n'


def needs_sudo_shim() -> bool:
    return os.geteuid() == 0 and shutil.which("sudo") is None


def read_env_file(path: Path) -> dict[str, str]:
    values = {}
    for line in path.read_text().splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    return values


def main(argv: list[str] | None = None) -> int:  # noqa: C901
    args = parse_args(argv)
    steps = plan(args)
    if args.dry_run:
        for i, step in enumerate(steps, 1):
            print(f"==> [{i}/{len(steps)}] {step.name}: {step.title}")
            for key, value in step.env.items():
                print(f"    env {key}={value}")
            print(step.script.strip("\n"))
        return 0

    # The exact glibc OBS is linked against: the digest-pinned manylinux_2_28
    # image's (#926). Anything else would silently move the AppImage's floor.
    libc = os.confstr("CS_GNU_LIBC_VERSION") or ""
    if libc != "glibc 2.28":
        print(f"==> refusing to build: {libc or 'unknown libc'}, expected exactly glibc 2.28 "
              "(build-aux/appimage/manylinux/Dockerfile)", file=sys.stderr)
        return 1
    shim = SUDO_SHIM if needs_sudo_shim() else ""
    with tempfile.TemporaryDirectory(prefix="obs-appimage-build-") as scratch:
        # Like Actions: GITHUB_ENV carries values to later steps; GITHUB_OUTPUT
        # stays the runner's when there is one (the configure step's `rust`).
        github_env = Path(scratch) / "env"
        github_env.touch()
        github_output = os.environ.get("GITHUB_OUTPUT") or str(Path(scratch) / "output")
        for i, step in enumerate(steps, 1):
            print(f"\n==> [{i}/{len(steps)}] {step.name}: {step.title}", flush=True)
            for stale in STALE.get(step.name, ()):
                path = ROOT / stale
                if path.is_dir() and not path.is_symlink():
                    shutil.rmtree(path)
                elif path.exists() or path.is_symlink():
                    path.unlink()
            env = {**os.environ, **read_env_file(github_env), **step.env,
                   "GITHUB_ENV": str(github_env), "GITHUB_OUTPUT": github_output}
            result = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", shim + step.script],
                                    cwd=ROOT, env=env)
            if result.returncode:
                print(f"==> FAILED: {step.name} ({step.title}) exited {result.returncode}", file=sys.stderr)
                return result.returncode

    if "appimage" in args.steps:
        images = sorted(ROOT.glob("OBS-Studio-*-linux-x86_64.AppImage*"))
        if args.output_dir is not None:
            out = args.output_dir if args.output_dir.is_absolute() else ROOT / args.output_dir
            out.mkdir(parents=True, exist_ok=True)
            images = [Path(shutil.move(str(image), out / image.name)) for image in images]
        for image in images:
            print(f"==> AppImage: {image}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
