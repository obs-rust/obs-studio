#!/usr/bin/env -S uv run --script
"""Manual safety probe for soldr's Rust cache; exits 1 on stale output.

    uv run ci/cache_source_probe.py --directory build_ci_audit/source-probe \
        --soldr /path/to/pinned/soldr --expected-version 0.9.35

Keeps the scratch crate, command logs and result.json for inspection. This
is an upstream-blocker experiment, not a required CI job. See #1286 and
https://github.com/zackees/zccache/issues/1943.
Qualification requires an unchanged cache hit followed by rejection of a
same-size/same-mtime compile error and an independent uncached control.
Each invocation has a finite --timeout (120 seconds by default); a timeout
keeps its partial log and an unsafe result. This probes Rust, not native GCC.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MARKER = "CACHE_SOURCE_PROBE"


def select_soldr(executable: str, expected_version: str | None, timeout: float) -> tuple[str, str]:
    if not math.isfinite(timeout) or timeout <= 0:
        raise ValueError("Timeout must be a finite positive number")
    # Resolve before changing cwd: uv can put a different soldr on PATH, and
    # relative paths otherwise name a different executable in the scratch crate.
    selected = shutil.which(executable) if not os.path.dirname(executable) else executable
    if selected is None:
        raise ValueError(f"Soldr executable not found: {executable}")
    executable = str(Path(selected).resolve())
    version = subprocess.run([executable, "--version"], capture_output=True, text=True,
                             check=True, timeout=timeout).stdout.strip()
    if expected_version and version != f"soldr {expected_version}":
        raise ValueError(f"Expected soldr {expected_version}, got {version}")
    return executable, version


def probe(directory: Path, executable: str, expected_version: str | None, *, timeout: float = 120) -> dict:
    executable, version = select_soldr(executable, expected_version, timeout)
    directory = directory.resolve()
    if directory.exists():
        raise ValueError(f"Use a new directory to preserve previous evidence: {directory}")
    (directory / "src").mkdir(parents=True)
    (directory / "Cargo.toml").write_text(
        '[package]\nname="obs-cache-source-probe"\nversion="0.1.0"\nedition="2021"\n[workspace]\n'
    )
    (directory / "Cargo.lock").write_text(
        'version = 3\n\n[[package]]\nname = "obs-cache-source-probe"\nversion = "0.1.0"\n'
    )
    (directory / "rust-toolchain.toml").write_bytes((ROOT / "rust-toolchain.toml").read_bytes())
    root = directory / "src/lib.rs"
    root.write_text("pub mod nested;\n")
    source = directory / "src/nested.rs"
    source.write_text(f'// compile_error!("{MARKER}");\npub fn ready() {{}}\n')
    env = dict(os.environ)
    env.pop("SOLDR_RUSTC_WRAPPER", None)
    env.update(SOLDR_PORCELAIN="0", CARGO_INCREMENTAL="0", CARGO_TERM_COLOR="never",
               CARGO_TARGET_DIR=str(directory / "target"))
    command = [executable, "cargo", "check", "--locked"]
    result = {"soldr": version, "command": command, "timeout_seconds": timeout, "safe": False}

    def persist() -> None:
        (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")

    def run(name: str, *, bypass: bool = False) -> dict:
        child_env = dict(env)
        if bypass:
            child_env["SOLDR_RUSTC_WRAPPER"] = "none"
        start = time.monotonic()
        timed_out = False
        try:
            child = subprocess.run(command, cwd=directory, env=child_env, capture_output=True,
                                   text=True, timeout=timeout)
            code, output = child.returncode, child.stdout + child.stderr
        except subprocess.TimeoutExpired as error:
            # TimeoutExpired may contain bytes even with text=True. Preserve the
            # captured output and a failed qualification rather than a traceback
            # that loses the command's evidence.
            def text(value: str | bytes | None) -> str:
                return value.decode(errors="replace") if isinstance(value, bytes) else value or ""

            code, output = None, text(error.stdout) + text(error.stderr)
            timed_out = True
        (directory / f"{name}.log").write_text(output)
        return {
            "returncode": code,
            "timed_out": timed_out,
            "seconds": round(time.monotonic() - start, 3),
            "probe_diagnostic": f"error: {MARKER}" in output,
            "cache_hit_reported": "[HIT" in output,
            "log": f"{name}.log",
        }

    warm = result["warm"] = run("warm")
    persist()
    if warm["returncode"] != 0:
        return result

    def force_cargo() -> None:
        # Cargo must invoke rustc even though the nested source's stamp is
        # unchanged. The root contents remain identical, so a correct cache
        # must detect the nested source's content change.
        now = time.time_ns() + 2_000_000_000
        os.utime(root, ns=(now, now))

    force_cargo()
    unchanged = result["unchanged"] = run("unchanged-replay")
    persist()
    if unchanged["returncode"] != 0 or not unchanged["cache_hit_reported"]:
        return result
    stamp = source.stat()
    good = source.read_bytes()
    source.write_bytes(b"   " + good[3:])
    os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
    assert source.stat().st_size == stamp.st_size
    assert source.stat().st_mtime_ns == stamp.st_mtime_ns

    force_cargo()
    cached = result["cached"] = run("cached-changed-source")
    persist()
    force_cargo()
    control = result["control"] = run("uncached-control", bypass=True)
    result["preserved_source_size_and_mtime"] = True
    result["safe"] = all(row["returncode"] not in (None, 0) and row["probe_diagnostic"]
                         for row in (cached, control))
    persist()
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--soldr", default="soldr", help="Explicit binary avoids a different soldr on uv's PATH")
    parser.add_argument("--expected-version", help="Refuse to measure a different soldr release")
    parser.add_argument("--timeout", type=float, default=120, help="Seconds allowed for each invocation")
    args = parser.parse_args()
    outcome = probe(args.directory, args.soldr, args.expected_version, timeout=args.timeout)
    print(json.dumps(outcome, indent=2))
    raise SystemExit(0 if outcome["safe"] else 1)
