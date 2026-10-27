#!/usr/bin/env -S uv run --script
"""Bootstrap the native obs-build tool, then enter it outside Cargo/Soldr's
process tree. `./native-build <args>` runs this file through uv.

    uv run --script ci/native_build.py --help
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parent.parent


def host_target() -> str:
    """The target triple this machine builds for, from the pinned toolchain."""
    try:
        result = subprocess.run(
            ["soldr", "--no-timestamp-lines", "rustc", "-vV"],
            capture_output=True,
            text=True,
        )
    except OSError as error:
        print(f"native-build: {error}", file=sys.stderr)
        return ""
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ").strip()
    return ""


def main(argv: list[str]) -> int:
    host = host_target()
    if not host:
        print("native-build: could not determine the Rust host target", file=sys.stderr)
        return 1
    output = Path(os.environ.get("OBS_NATIVE_TOOL_OUTPUT") or REPOSITORY / "build_cargo" / "tool")
    # Match the native Rust consumer safeguard in docs/ci-tooling.md. The
    # tool inherits it too, as the exported shell variable did.
    os.environ["SOLDR_RUSTC_WRAPPER"] = "none"
    build = subprocess.run(
        [
            "soldr", "cargo", "build", "--locked",
            "--manifest-path", str(REPOSITORY / "Cargo.toml"),
            "-p", "obs-build", "--bin", "obs-build",
            "--target", host, "--target-dir", str(output),
        ],
        env=os.environ.copy(),
    )
    if build.returncode != 0:
        return build.returncode
    tool = output / host / "debug" / "obs-build"
    if "-windows-" in host:
        tool = tool.with_suffix(".exe")
    try:
        os.execv(str(tool), [str(tool), *argv[1:]])
    except OSError as error:
        print(f"native-build: {error}", file=sys.stderr)
        return 127  # what a shell reports for a missing command
    return 1  # unreachable: execv only returns by raising


if __name__ == "__main__":
    sys.exit(main(sys.argv))
