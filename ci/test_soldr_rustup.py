#!/usr/bin/env -S uv run --script
"""build-project.yaml installs the Rust toolchain through soldr, never bare `rustup`.

The `macos-build` and `windows-build` jobs install the toolchain `rust-toolchain.toml`
pins and add a cross-target standard library. Both go through `soldr rustup` so the
pinned toolchain, cache and Dylint wiring apply; a bare `rustup` bypasses Soldr's
Rust toolchain wrapper (ci-lint RUST-001/TOOL-001). The sanctionsised form is the
one `release-rust.yaml` already uses (`soldr rustup target add <triple>`).

    uv run python -m unittest ci/test_soldr_rustup.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"

# A `rustup` token not preceded by `soldr ` on the same line.
BARE_RUSTUP = re.compile(r"(?<!soldr )\brustup\b")


def bare_rustup_lines(text: str) -> list[str]:
    """Every non-comment line that invokes `rustup` without the `soldr ` wrapper."""
    return [
        line
        for line in text.splitlines()
        if not line.lstrip().startswith("#") and BARE_RUSTUP.search(line)
    ]


class NoBareRustup(unittest.TestCase):
    def test_build_project_wraps_rustup_in_soldr(self):
        offenders = bare_rustup_lines(WORKFLOW.read_text())
        self.assertEqual(
            offenders,
            [],
            "install the toolchain through `soldr rustup` (RUST-001/TOOL-001): "
            + "; ".join(offenders),
        )

    def test_cross_targets_are_installed_through_soldr(self):
        text = WORKFLOW.read_text()
        self.assertIn("soldr rustup toolchain install", text)
        self.assertIn("soldr rustup target add x86_64-apple-darwin", text)
        self.assertIn("soldr rustup target add aarch64-pc-windows-msvc", text)


if __name__ == "__main__":
    unittest.main()
