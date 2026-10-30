#!/usr/bin/env -S uv run --script
"""Exercise host dependency commands without network or package installation (#1269)."""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"
STEPS = ROOT / "ci/ci_gate_steps.py"
# The `Install Runtime Test Infrastructure` step of ci.yml is one Python call
# (#2851), so that job is driven through the module; the jobs still carrying a
# `run: |` block are executed from the workflow text.
PORTED_CALL = "ci/ci_gate_steps.py install-runtime-deps"
SUDO_STUB = """#!/bin/sh
for argument in "$@"; do
    case "$argument" in
        *host_apt_mirrors.py) [ "$APT_FAIL" = mirror ] && exit 42; exit 0 ;;
    esac
done
exec "$@"
"""
APT_STUB = """#!/bin/sh
printf '%s\\0' "$@" >> "$APT_TRACE"
printf '\\n' >> "$APT_TRACE"
for argument in "$@"; do [ "$argument" = "$APT_FAIL" ] && exit 42; done
exit 0
"""
RUNTIME_PACKAGES = """flatpak xvfb dbus-daemon pipewire wireplumber pipewire-pulse pipewire-bin
    pulseaudio-utils gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-pipewire
    libgl1-mesa-dri gcc pkg-config libglib2.0-dev libpipewire-0.3-dev libpulse-dev
    libcurl4-openssl-dev ffmpeg gdb binutils"""
PACKAGES = {
    "flatpak-runtime-test": RUNTIME_PACKAGES,
    "flatpak-runtime-reuse": RUNTIME_PACKAGES,
    "rust-tests-linux": "uuid-dev libglib2.0-dev libxkbcommon-dev",
    "linux-qt-frontend": """build-essential cmake ninja-build pkg-config ccache git curl zstd
        libcurl4-openssl-dev libjansson-dev libx11-dev libx11-xcb-dev libxcb1-dev
        libxcb-randr0-dev libxcb-shm0-dev libxcb-xinerama0-dev libxcb-composite0-dev
        libxcb-xfixes0-dev libxcb-xinput-dev libxinerama-dev libxcomposite-dev libxdamage-dev
        libgl-dev libglvnd-dev libgles2-mesa-dev libwayland-dev libxkbcommon-dev
        libpulse-dev libasound2-dev libpipewire-0.3-dev libpci-dev libdrm-dev libudev-dev
        libv4l-dev libva-dev libfontconfig-dev libfreetype-dev libspeexdsp-dev libsimde-dev
        uthash-dev nlohmann-json3-dev libwebsocketpp-dev libasio-dev libqrcodegencpp-dev
        libcmocka-dev libxss-dev extra-cmake-modules xvfb xauth weston libdbus-1-dev
        libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 libxcb-render-util0
        libxcb-xkb1 libxkbcommon-x11-0 libxcb-shape0 libxcb-sync1 libegl1""",
}


def dependency_script(job: str) -> str:
    workflow = WORKFLOW.with_name("ci.yml") if job == "flatpak-runtime-reuse" else WORKFLOW
    text = workflow.read_text(encoding="utf-8")
    body = re.search(rf"^  {job}:\n(.*?)(?=^  [a-z0-9-]+:|\Z)", text, re.S | re.M).group(1)
    name = ("Install Runtime Test Infrastructure" if job.startswith("flatpak-runtime-") else
            "Install libuuid and GIO" if job == "rust-tests-linux" else "Install Build Dependencies 📦")
    step = body.split(f"      - name: {name}\n", 1)[1]
    run = re.search(r"^        run: (.*)\n", step, re.M)
    if run.group(1) != "|":
        return run.group(1)
    block = re.match(r"((?:          [^\n]*\n|\n)*)", step[run.end():]).group(1)
    return textwrap.dedent(block)


class HostAptFailoverTest(unittest.TestCase):
    def execute(self, job: str, fail: str = "") -> tuple[int, list[list[str]]]:
        script = dependency_script(job)
        if PORTED_CALL in script:
            return self.execute_module(fail)
        mock = r'''
: > "$APT_TRACE"
sudo() {
    if [[ "$*" == *host_apt_mirrors.py* ]]; then
        # Mirror rewriting is separately executed on isolated fixture files.
        if [[ "$APT_FAIL" == mirror ]]; then return 42; fi
        return 0
    fi
    "$@"
}
apt-get() {
    printf '%s\0' "$@" >> "$APT_TRACE"
    printf '\n' >> "$APT_TRACE"
    local arg
    for arg in "$@"; do
        if [[ "$arg" == "$APT_FAIL" ]]; then return 42; fi
    done
    return 0
}
'''
        with tempfile.TemporaryDirectory() as tmp:
            trace = Path(tmp) / "calls"
            result = subprocess.run(
                ["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", "-c", mock + dependency_script(job)],
                env={**os.environ, "APT_TRACE": str(trace), "APT_FAIL": fail},
                capture_output=True, text=True, timeout=10,
            )
            calls = [line.rstrip(b"\0").decode().split("\0") for line in trace.read_bytes().splitlines()]
        return result.returncode, calls

    def execute_module(self, fail: str) -> tuple[int, list[list[str]]]:
        """`ci_gate_steps.py install-runtime-deps` under the same two stubs."""

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            binaries = root / "bin"
            binaries.mkdir()
            for name, text in (("sudo", SUDO_STUB), ("apt-get", APT_STUB)):
                path = binaries / name
                path.write_text(text, encoding="utf-8")
                path.chmod(0o755)
            trace = root / "calls"
            trace.touch()
            result = subprocess.run(
                [sys.executable, str(STEPS), "install-runtime-deps"],
                env={
                    **os.environ,
                    "APT_TRACE": str(trace),
                    "APT_FAIL": fail,
                    "PATH": f"{binaries}{os.pathsep}{os.environ.get('PATH', '')}",
                },
                capture_output=True,
                text=True,
                timeout=30,
            )
            calls = [line.rstrip(b"\0").decode().split("\0") for line in trace.read_bytes().splitlines()]
        return result.returncode, calls

    def test_acquire_bounds_and_complete_package_inputs(self):
        for job, packages in PACKAGES.items():
            with self.subTest(job=job):
                code, calls = self.execute(job)
                self.assertEqual(code, 0)
                self.assertEqual(len(calls), 2)
                for call, action in zip(calls, ("update", "install"), strict=True):
                    options = []
                    remaining = []
                    index = 0
                    while index < len(call):
                        if call[index] == "-o":
                            options.append(call[index + 1])
                            index += 2
                        else:
                            remaining.append(call[index])
                            index += 1
                    self.assertCountEqual(options, ["Acquire::Retries=1", "Acquire::http::Timeout=15", "Acquire::https::Timeout=15"])
                    expected = (["update", "-qq"] if job.startswith("flatpak-runtime-") else ["update"]) if action == "update" else ["install", "-y"] + (["--no-install-recommends"] if job != "rust-tests-linux" else []) + packages.split()
                    self.assertEqual(remaining, expected)

    def test_update_and_install_failures_propagate(self):
        for job in PACKAGES:
            for action, count in (("mirror", 0), ("update", 1), ("install", 2)):
                with self.subTest(job=job, action=action):
                    code, calls = self.execute(job, action)
                    self.assertEqual(code, 42)
                    self.assertEqual(len(calls), count)


if __name__ == "__main__":
    unittest.main()
