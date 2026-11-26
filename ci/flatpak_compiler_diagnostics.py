#!/usr/bin/env -S uv run --script
"""Opt-in zccache journal observations; raw commands and environment stay on the runner."""

import argparse
import copy
from collections import Counter
import json
from pathlib import Path
import re

MAX_BYTES = 16 * 1024 * 1024
OUTCOMES = frozenset(('hit', 'miss', 'passthrough', 'error', 'cached_error'))
COMPILER_OPTIONS = re.compile(
    r"^(?:preprocessing-only flag: |unmodeled compiler side output requested by )"
    r"(--?[\w-]{1,80}(?:,-?[\w-]{1,80})*)(?:[= ].*)?$"
)


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
    modules[0].setdefault("build-options", {}).setdefault("env", {})["OBS_FLATPAK_ZCCACHE_JOURNAL"] = "1"
    return result


def summarize(log):
    if len(log.encode()) > MAX_BYTES:
        raise ValueError("Compiler journal exceeds 16 MiB observation limit")
    rows = []
    results, options = Counter(), Counter()
    for line in log.splitlines():
        if not line:
            continue
        call = json.loads(line)
        outcome = call.get('outcome')
        outcome = outcome if outcome in OUTCOMES else 'other'
        args = call.get('args', [])
        source = '(unknown)'
        if '-c' in args:
            operands = args[args.index('-c') + 1:]
            if operands and not operands[0].startswith('-'):
                source = source_label(operands[0])
        span = call.get('latency_ns', 0)
        flag = COMPILER_OPTIONS.fullmatch(call.get('passthrough_reason') or '')
        if flag:
            options[flag[1]] += 1
        results[outcome] += 1
        rows.append({'source': source, 'log_span_seconds': max(0, span) / 1e9,
                     'results': [outcome], 'unsupported_options': [flag[1]] if flag else []})
    return {'calls': len(rows), 'results': dict(results), 'unsupported_options': dict(options),
            'slowest_calls': sorted(rows, key=lambda c: c['log_span_seconds'], reverse=True)[:40],
            'uncacheable_calls': [c for c in rows if 'passthrough' in c['results']][:80],
            'limits': 'Request spans overlap and are not CPU or job time. Raw commands, reasons and environment are not published.'}


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
        files = sorted({p.resolve() for p in (args.state / "build").glob("obs*/flatpak-compiler.jsonl") if p.is_file()})
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
            raise ValueError("No zccache invocations observed")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n")
        print(f"Observed {result['calls']} compiler calls; sanitized report: {args.output.name}")


if __name__ == "__main__":
    main()
