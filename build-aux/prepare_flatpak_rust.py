#!/usr/bin/env -S uv run --script
"""Vendor the Rust workspace for flatpak-builder, whose sandbox has no network.

    uv run --script build-aux/prepare_flatpak_rust.py

`cargo vendor` writes both the vendor tree and the source configuration. A cold
soldr bootstrap can print rustup's welcome message to stdout before that
configuration, so the raw output is kept and the TOML is taken from the first
`[source.` table, with the vendor directory rewritten to the sandbox path
flatpak-builder mounts.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / ".flatpak-rust"
SANDBOX_VENDOR = "/run/build/obs/.flatpak-rust/vendor"


def vendor_config(raw: str) -> str:
    """The `cargo vendor` configuration, sandbox-pathed and already parsed."""
    start = raw.find("[source.")
    if start < 0:
        raise SystemExit("cargo vendor did not emit source configuration")
    config = re.sub(r'^directory = ".*"$', f'directory = "{SANDBOX_VENDOR}"',
                    raw[start:], flags=re.MULTILINE)
    parsed = tomllib.loads(config)
    if parsed["source"]["vendored-sources"]["directory"] != SANDBOX_VENDOR:
        raise SystemExit("cargo vendor configuration does not select the sandbox vendor directory")
    return config


def main() -> int:
    (WORK / "cargo").mkdir(parents=True, exist_ok=True)
    stdout = WORK / "vendor-config.stdout"
    with stdout.open("w", encoding="utf-8") as sink:
        result = subprocess.run(
            ["soldr", "cargo", "vendor", "--locked", str(WORK / "vendor")],
            cwd=ROOT, stdout=sink,
        )
    if result.returncode != 0:
        return result.returncode
    config = vendor_config(stdout.read_text(encoding="utf-8"))
    (WORK / "cargo" / "config.toml").write_text(config, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
