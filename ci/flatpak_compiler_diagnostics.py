#!/usr/bin/env -S uv run --script
"""Opt-in ccache observations; raw commands and environment stay on the runner."""

import argparse
import copy
from collections import Counter
from datetime import datetime
import json
from pathlib import Path
import re
import shlex

MAX_BYTES = 16 * 1024 * 1024
LINE = re.compile(r"^\[([\dT:.+-]+)\s+(\d+)\s*\]\s+(.*)$")
UNSUPPORTED = re.compile(r"^(?:Unsupported compiler option: (\S+)|(?:Compiler option |Option )?(-\S+) is unsupported(?: .*)?)$")


def source_label(source):
    if source.startswith("/run/build/obs/"):
        source = source.removeprefix("/run/build/obs/")
    if not source.startswith("/") and ".." not in Path(source).parts and re.fullmatch(r"[\w./+-]{1,240}", source):
        return source
    return "(external)"


def configure(manifest):
    result = copy.deepcopy(manifest)
    modules = [m for m in result.get("modules", []) if isinstance(m, dict) and m.get("name") == "obs"]
    if len(modules) != 1:
        raise ValueError("Expected exactly one obs source module")
    modules[0].setdefault("build-options", {}).setdefault("env", {})["CCACHE_LOGFILE"] = "/run/build/obs/flatpak-compiler.log"
    return result


def summarize(log):
    if len(log.encode()) > MAX_BYTES:
        raise ValueError("Compiler log exceeds 16 MiB observation limit")
    active, calls = {}, []
    options = Counter()
    for line in log.splitlines():
        match = LINE.match(line)
        if not match:
            continue
        stamp, pid, message = match.groups()
        try:
            timestamp = datetime.fromisoformat(stamp)
        except ValueError:
            continue
        if "=== CCACHE " in message and "STARTED" in message:
            if pid in active:
                calls.append(active.pop(pid))
            active[pid] = {"start": timestamp, "end": timestamp, "source": "(unknown)", "results": set(), "options": set()}
        if pid not in active:
            continue
        call = active[pid]
        call["end"] = timestamp
        if message.startswith("Source file: "):
            call["source"] = source_label(message.removeprefix("Source file: "))
        elif message.startswith("Command line: "):
            # Argument rejection happens before ccache emits "Source file".
            # Keep only the source operand after -c; discard every other value.
            try:
                args = shlex.split(message.removeprefix("Command line: "))
                index = args.index("-c")
                if index + 1 < len(args) and not args[index + 1].startswith("-"):
                    call["source"] = source_label(args[index + 1])
            except ValueError:
                pass
        elif message.startswith("Result: "):
            value = message.removeprefix("Result: ")
            if re.fullmatch(r"[a-z_]{1,80}", value):
                call["results"].add(value)
        elif match := UNSUPPORTED.fullmatch(message):
            option = next(value for value in match.groups() if value).split("=", 1)[0]
            if re.fullmatch(r"--?[\w-]{1,80}(?:,-?[\w-]{1,80})*", option):
                options[option] += 1
                call["options"].add(option)
    calls.extend(active.values())
    results = Counter(result for call in calls for result in call["results"])
    rows = [{"source": c["source"], "log_span_seconds": max(0, (c["end"] - c["start"]).total_seconds()),
             "results": sorted(c["results"]), "unsupported_options": sorted(c["options"])} for c in calls]
    return {"calls": len(calls), "results": dict(results), "unsupported_options": dict(options),
            "slowest_calls": sorted(rows, key=lambda c: c["log_span_seconds"], reverse=True)[:40],
            "uncacheable_calls": [c for c in rows if "unsupported_compiler_option" in c["results"]][:80],
            "limits": "Log spans are wall intervals, not CPU time. An uncacheable call can exec the compiler after logging ends; its span excludes that execution. Concurrent spans must not be summed as job time. Raw commands/environment are not published."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["configure", "summarize"])
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--state", type=Path, default=Path(".flatpak-builder"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.operation == "configure":
        if args.manifest is None:
            parser.error("configure requires --manifest")
        args.manifest.write_text(json.dumps(configure(json.loads(args.manifest.read_text())), indent=4) + "\n")
    else:
        if args.output is None:
            parser.error("summarize requires --output")
        files = sorted({p.resolve() for p in (args.state / "build").glob("obs*/flatpak-compiler.log") if p.is_file()})
        if not files or len(files) > 8:
            raise ValueError("Expected 1–8 compiler logs in OBS build directories")
        chunks, remaining = [], MAX_BYTES
        for path in files:
            with path.open("rb") as stream:
                data = stream.read(remaining + 1)
            if len(data) > remaining:
                raise ValueError("Compiler logs exceed 16 MiB observation limit")
            remaining -= len(data)
            chunks.append(data.decode("utf-8", errors="replace"))
        result = summarize("\n".join(chunks))
        if not result["calls"]:
            raise ValueError("No ccache invocations observed")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n")
        print(f"Observed {result['calls']} compiler calls; sanitized report: {args.output.name}")


if __name__ == "__main__":
    main()
