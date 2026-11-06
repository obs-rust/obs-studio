#!/usr/bin/env -S uv run --script
"""Opt-in LLDB launch and evidence preservation for the packaged macOS probe.

Set OBS_MACOS_STARTUP_LLDB=1 (workflow repository variable of the same name)
for the original launch to run under LLDB. There is no diagnostic retry.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path


def launch_command(command: list[str], mode: str, env=None) -> list[str]:
    env = os.environ if env is None else env
    if env.get("OBS_MACOS_STARTUP_LLDB") != "1":
        return command
    driver = Path(__file__).with_name("macos_startup_lldb.py").resolve()
    evidence = Path(env.get("OBS_MACOS_STARTUP_EVIDENCE", "macos-startup-evidence"))
    evidence.mkdir(parents=True, exist_ok=True)
    # Arguments travel through the environment, never through LLDB's parser.
    os.environ["OBS_LLDB_LOG"] = str((evidence / f"lldb-{mode}.log").resolve())
    return [
        "xcrun",
        "lldb",
        "--batch",
        "-o",
        f'command script import "{driver}"',
        "--",
        *command,
    ]


def collect(config: Path | None = None, mode: str = "final") -> None:
    # Evidence collection cannot replace the probe's original success/failure.
    try:
        _collect(config, mode)
    except OSError as error:
        print(f"::warning::Cannot collect startup evidence: {error}")


def _collect(config: Path | None, mode: str) -> None:
    evidence = Path(
        os.environ.get("OBS_MACOS_STARTUP_EVIDENCE", "macos-startup-evidence")
    )
    evidence.mkdir(parents=True, exist_ok=True)
    report_patterns = ("**/*.ips", "**/*.crash")
    sources = [
        (Path.home() / "Library/Logs/DiagnosticReports", "crash-reports", report_patterns),
        (Path("/Library/Logs/DiagnosticReports"), "system-crash-reports", report_patterns),
    ]
    if config is not None:
        sources.append((config, f"app-{mode}", ("**/*.txt",)))
    for source, name, patterns in sources:
        for pattern in patterns:
            for report in source.glob(pattern):
                destination = evidence / name / report.relative_to(source)
                try:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(report, destination)
                except OSError as error:
                    print(f"::warning::Cannot preserve {report}: {error}")
    (evidence / "collection.txt").write_text(
        "Collected available macOS crash reports and frontend logs.\n"
    )


if __name__ == "__main__":
    collect()
