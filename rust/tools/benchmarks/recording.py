#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["websockets==15.0.1"]
# ///
"""Full-app recording evidence for the 5% recording release gate (#1305).

One run records from an OBS that is already running with the scene and
profile under test, and writes one JSON file:

    uv run rust/tools/benchmarks/recording.py run --log <OBS log> --pid <OBS pid> \\
        --libobs rust --prefix /opt/obs --encoder obs-nvenc --seconds 120 \\
        --ws-config <plugin_config/obs-websocket/config.json> --output rust-1.json

Before it measures, the run must pass ci/recording_evidence_check.py
(hardware renderer, the expected libobs mapped). It then starts the
recording, waits --warmup seconds, samples GetStats every second, reads the
recording output's counters at the start and the end, stops the recording
and checks that the expected encoder wrote the settings lines after
"Recording Start" in the log. No profiler runs. The script does not switch
between the C and Rust libobs: restart OBS with the other library between
runs (interleave C and Rust) and run it again.

The report compares at least 5 runs per backend that have the same
conditions (video settings, recording profile, scene, source, renderer and
graphics module) and one libobs build per backend. GetStats counts the
frames of all video outputs (e.g. an NDI output next to the recording), so
the encoder skip rate comes from the recording output's own counters:

    uv run rust/tools/benchmarks/recording.py report c-*.json rust-*.json [--markdown out.md]

It gives the median and range per backend and the Rust/C ratio. The gate
(release-criteria.md): the Rust median frame render time is at most 5%
above C, and no skip rate (render, output, encoder) is more than 5% above
C. When C has no skipped frames, Rust must have none.
"""

from __future__ import annotations

import argparse
import asyncio
import datetime
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BOUND = 1.05
COUNTERS = ("renderTotalFrames", "renderSkippedFrames", "outputTotalFrames", "outputSkippedFrames")
METRICS = (
    "frame_render_ms_median",
    "frame_render_ms_p95",
    "cpu_percent_median",
    "render_skip_rate",
    "output_skip_rate",
    "encoder_skip_rate",
    "rendered_frames",
)
SKIP_RATES = ("render_skip_rate", "output_skip_rate", "encoder_skip_rate")
PROFILE_KEYS = (
    ("Output", "Mode"),
    ("SimpleOutput", "RecEncoder"),
    ("SimpleOutput", "RecQuality"),
    ("SimpleOutput", "RecFormat2"),
    ("SimpleOutput", "VBitrate"),
    ("AdvOut", "RecType"),
    ("AdvOut", "RecEncoder"),
    ("AdvOut", "RecFormat2"),
)
RECORD_KINDS = {"ffmpeg_muxer", "mp4_output", "mov_output"}
MIN_SAMPLES = 10


class RunError(Exception):
    pass


def _p95(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(0.95 * (len(ordered) - 1)))]


def run_metrics(samples: list[dict], rec_start: dict, rec_end: dict) -> dict:
    """Metrics of one run from per-second GetStats samples and the recording
    output's GetOutputStatus at the start and the end of the window."""
    if len(samples) < MIN_SAMPLES:
        raise RunError(f"{len(samples)} samples, need at least {MIN_SAMPLES}")
    if not rec_end.get("outputActive"):
        raise RunError("the recording output was not active at the end of the window")
    for key in COUNTERS:
        for a, b in zip(samples, samples[1:]):
            if b[key] < a[key]:
                raise RunError(f"{key} went back ({a[key]} -> {b[key]}): OBS restarted or reset")
    first, last = samples[0], samples[-1]
    delta = {key: last[key] - first[key] for key in COUNTERS}
    rendered = delta["renderTotalFrames"]
    if rendered <= 0:
        raise RunError("no frames were rendered in the window")
    encoded = rec_end["outputTotalFrames"] - rec_start["outputTotalFrames"]
    enc_skipped = rec_end["outputSkippedFrames"] - rec_start["outputSkippedFrames"]
    if encoded <= 0 or enc_skipped < 0:
        raise RunError(f"recording counters are not usable: {encoded} frames, {enc_skipped} skipped")
    frame_ms = [s["averageFrameRenderTime"] for s in samples]
    cpu = [s["cpuUsage"] for s in samples]
    output = delta["outputTotalFrames"]
    return {
        "duration_s": round(last["t"] - first["t"], 3),
        "samples": len(samples),
        "rendered_frames": rendered,
        "render_skipped": delta["renderSkippedFrames"],
        "render_skip_rate": delta["renderSkippedFrames"] / rendered,
        "output_frames": output,
        "output_skipped": delta["outputSkippedFrames"],
        "output_skip_rate": delta["outputSkippedFrames"] / output if output > 0 else 0.0,
        "encoder_frames": encoded,
        "encoder_skipped": enc_skipped,
        "encoder_skip_rate": enc_skipped / encoded,
        "frame_render_ms_median": statistics.median(frame_ms),
        "frame_render_ms_p95": _p95(frame_ms),
        "cpu_percent_median": statistics.median(cpu),
        "fps_median": statistics.median(s["activeFps"] for s in samples),
    }


def _conditions(run: dict) -> str:
    identity = run["identity"]
    return json.dumps(
        {
            "conditions": run["conditions"],
            "renderer": identity["renderer"],
            "graphics": identity["graphics"]["sha256"],
        },
        sort_keys=True,
    )


def report(runs: list[dict], min_runs: int = 5) -> dict:
    groups: dict[str, list[dict]] = {"c": [], "rust": []}
    for run in runs:
        groups[run["identity"]["libobs"]["backend"]].append(run)
    for backend, group in groups.items():
        if len(group) < min_runs:
            raise RunError(f"{len(group)} {backend} runs, need at least {min_runs} per backend")
        builds = {r["identity"]["libobs"]["sha256"] for r in group}
        if len(builds) > 1:
            raise RunError(f"more than one {backend} libobs build in the runs: {sorted(builds)}")
    keys = {_conditions(r) for r in runs}
    if len(keys) > 1:
        raise RunError(f"conditions differ between runs ({len(keys)} different sets)")
    metrics = {}
    for name in METRICS:
        row = {}
        for backend, group in groups.items():
            values = [r["metrics"][name] for r in group]
            row[backend] = {"median": statistics.median(values), "min": min(values), "max": max(values)}
        c = row["c"]["median"]
        row["ratio"] = row["rust"]["median"] / c if c else None
        metrics[name] = row
    ft = metrics["frame_render_ms_median"]["ratio"]
    frame_time = {"ratio": ft, "bound": BOUND, "pass": ft is not None and ft <= BOUND}
    drops = {}
    for name in SKIP_RATES:
        c, rust = metrics[name]["c"]["median"], metrics[name]["rust"]["median"]
        drops[name] = {"c": c, "rust": rust, "pass": rust <= c * BOUND if c else rust == 0}
    dropped = {"rates": drops, "pass": all(d["pass"] for d in drops.values())}
    first = runs[0]
    return {
        "runs": {backend: len(group) for backend, group in groups.items()},
        "conditions": first["conditions"],
        "renderer": first["identity"]["renderer"],
        "libobs": {b: g[0]["identity"]["libobs"] for b, g in groups.items()},
        "metrics": metrics,
        "gate": {"frame_time": frame_time, "dropped_frames": dropped},
        "pass": frame_time["pass"] and dropped["pass"],
    }


def _num(name: str, value: float) -> str:
    if name.endswith("_rate"):
        return f"{value * 100:.3f}%"
    if name == "rendered_frames":
        return f"{value:.0f}"
    return f"{value:.3f}"


def format_report(rep: dict) -> str:
    def cell(name: str, v: dict) -> str:
        return f"{_num(name, v['median'])} ({_num(name, v['min'])}-{_num(name, v['max'])})"

    lines = [
        f"Runs: {rep['runs']['c']} C, {rep['runs']['rust']} Rust. Renderer: "
        f"{rep['renderer']['api']} {rep['renderer']['adapter']} ({rep['renderer']['version']}).",
        "",
        "| metric | C median (range) | Rust median (range) | Rust/C |",
        "|---|---|---|---|",
    ]
    for name, row in rep["metrics"].items():
        ratio = "n/a" if row["ratio"] is None else f"{row['ratio']:.3f}"
        lines.append(f"| {name} | {cell(name, row['c'])} | {cell(name, row['rust'])} | {ratio} |")
    ft = rep["gate"]["frame_time"]
    ratio = "n/a" if ft["ratio"] is None else f"{ft['ratio']:.3f}"
    lines += [
        "",
        f"- Frame time: {'PASS' if ft['pass'] else 'FAIL'} (Rust/C {ratio}, bound {ft['bound']}).",
    ]
    for name, d in rep["gate"]["dropped_frames"]["rates"].items():
        lines.append(
            f"- {name}: {'PASS' if d['pass'] else 'FAIL'} (C {_num(name, d['c'])}, Rust {_num(name, d['rust'])})."
        )
    lines += ["", f"Result: {'PASS' if rep['pass'] else 'FAIL'}"]
    return "\n".join(lines) + "\n"


def encoder_lines(log: str, expect: str, before: int = 40) -> list[str]:
    """The last recording's log lines: OBS logs the encoder settings before
    the "Recording Start" marker and the frame summary after it. Take up to
    `before` lines back (not past an earlier "====" marker) and the lines up
    to "Recording Stop". The expected encoder must appear in them."""
    lines = log.splitlines()
    starts = [i for i, line in enumerate(lines) if "==== Recording Start" in line]
    if not starts:
        raise RunError("no '==== Recording Start' line in the log")
    start = starts[-1]
    first = start
    while first > 0 and start - first < before and "==== " not in lines[first - 1]:
        first -= 1
    block = lines[first:start]
    for line in lines[start:]:
        block.append(line)
        if "==== Recording Stop" in line:
            break
    if not any(expect in line for line in block[: start - first]):
        raise RunError(f"the encoder {expect!r} is not in the log lines before Recording Start")
    return block


# --- Live run (needs a running OBS) ------------------------------------------


def _evidence(log: Path, pid: int, libobs: str, prefix: Path | None) -> dict:
    sys.path.insert(0, str(ROOT / "ci"))
    import recording_evidence_check as check

    if Path(f"/proc/{pid}/maps").exists():
        maps = Path(f"/proc/{pid}/maps").read_text()
    else:
        maps = subprocess.run(["vmmap", "-w", str(pid)], capture_output=True, text=True, check=True).stdout
    try:
        return {
            "renderer": check.renderer(log.read_text(errors="replace")),
            **check.libraries(maps, expect=libobs, prefix=prefix),
        }
    except check.CheckError as e:
        raise RunError(f"evidence check failed: {e}") from e


def _password(ws_config: Path | None) -> str | None:
    if ws_config:
        config = json.loads(ws_config.read_text())
        return config.get("server_password") if config.get("auth_required", True) else None
    return os.environ.get("OBS_WS_PASSWORD")


def _host() -> dict:
    cpu = platform.processor()
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    return {"platform": platform.platform(), "cpu": cpu, "cpus": os.cpu_count()}


def source_version(log: str) -> str | None:
    """The build's version from the log header, e.g. 33.0.0-beta6-1609-g9f573a9b3."""
    for line in log.splitlines()[:100]:
        if "based on OBS " in line:
            return line.split("based on OBS ", 1)[1].split(" ")[0]
    return None


async def _record(args: argparse.Namespace) -> dict:
    sys.path.insert(0, str(ROOT / "test/websocket-tier4"))
    from obsws import SUB_NONE, ObsWsClient

    identity = _evidence(args.log, args.pid, args.libobs, args.prefix)
    client = ObsWsClient(args.url)
    hello = await asyncio.wait_for(client.connect(), timeout=10)
    if hello is None:
        raise RunError("obs-websocket handshake did not complete")
    await client.identify(
        password=_password(args.ws_config),
        auth_challenge=hello["d"].get("authentication"),
        event_subscriptions=SUB_NONE,
    )
    if await client.wait_op(2, timeout=5) is None:
        raise RunError("obs-websocket authentication failed")

    async def request(name: str, data: dict | None = None) -> dict:
        result = (await client.request(name, data))["d"]
        if not result["requestStatus"]["result"]:
            raise RunError(f"{name} failed: {result['requestStatus']}")
        return result.get("responseData", {})

    try:
        if (await request("GetRecordStatus"))["outputActive"]:
            raise RunError("a recording is already active")
        version = await request("GetVersion")
        profile = {}
        for category, name in PROFILE_KEYS:
            value = await request("GetProfileParameter", {"parameterCategory": category, "parameterName": name})
            profile[f"{category}/{name}"] = value.get("parameterValue")
        conditions = {
            "video": await request("GetVideoSettings"),
            "profile": profile,
            "scene": (await request("GetCurrentProgramScene"))["currentProgramSceneName"],
            "source": source_version(args.log.read_text(errors="replace")),
            "obs_version": version.get("obsVersion"),
        }
        started = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
        await request("StartRecord")
        output = None
        try:
            for _ in range(50):
                active = [
                    o for o in (await request("GetOutputList"))["outputs"]
                    if o["outputActive"] and o["outputKind"] in RECORD_KINDS
                ]
                if len(active) == 1:
                    output = active[0]["outputName"]
                    break
                await asyncio.sleep(0.2)
            if output is None:
                raise RunError("the recording output did not become active")
            await asyncio.sleep(args.warmup)
            rec_start = await request("GetOutputStatus", {"outputName": output})
            samples = []
            end = time.monotonic() + args.seconds
            while True:
                stats = await request("GetStats")
                samples.append({"t": time.monotonic(), **{k: stats[k] for k in (
                    "cpuUsage", "activeFps", "averageFrameRenderTime", *COUNTERS)}})
                if samples[-1]["t"] >= end:
                    break
                await asyncio.sleep(max(0.0, min(1.0, end - time.monotonic())))
            rec_end = await request("GetOutputStatus", {"outputName": output})
        finally:
            stopped = await request("StopRecord")
        await asyncio.sleep(1)
        recording = Path(stopped.get("outputPath", ""))
        file = {"path": str(recording), "bytes": recording.stat().st_size if recording.is_file() else None}
        try:
            if shutil.which("ffprobe") and recording.is_file():
                probe = subprocess.run(
                    ["ffprobe", "-v", "error", "-select_streams", "v:0", "-count_packets",
                     "-show_entries", "stream=codec_name,nb_read_packets", "-of", "json", str(recording)],
                    capture_output=True, text=True,
                )
                file["ffprobe"] = json.loads(probe.stdout or "{}") if probe.returncode == 0 else probe.stderr.strip()
            else:
                file["ffprobe"] = "not checked (no ffprobe)"
            return {
                "started": started,
                "host": _host(),
                "identity": identity,
                "conditions": conditions,
                "recording_output": output,
                "encoder_log": encoder_lines(args.log.read_text(errors="replace"), args.encoder),
                "file": file,
                "metrics": run_metrics(samples, rec_start, rec_end),
                "samples": samples,
            }
        finally:
            if args.delete_recording and recording.is_file():
                recording.unlink()
                file["deleted"] = True
    finally:
        await client.close_conn()


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("run", help="record once from a running OBS and write the run JSON")
    r.add_argument("--log", type=Path, required=True, help="the running OBS's log file")
    r.add_argument("--pid", type=int, required=True, help="the running OBS's process ID")
    r.add_argument("--libobs", choices=("c", "rust"), required=True, help="expected backend")
    r.add_argument("--prefix", type=Path, help="install prefix every OBS library must come from")
    r.add_argument("--encoder", required=True, help="text the encoder's log lines contain, e.g. obs-nvenc")
    r.add_argument("--seconds", type=float, default=120, help="measurement window (default 120)")
    r.add_argument("--warmup", type=float, default=10, help="seconds after the start before measuring")
    r.add_argument("--url", default="ws://127.0.0.1:4455")
    r.add_argument("--ws-config", type=Path, help="obs-websocket config.json (else OBS_WS_PASSWORD)")
    r.add_argument("--delete-recording", action="store_true", help="delete the file after the checks")
    r.add_argument("--output", type=Path, required=True)
    p = sub.add_parser("report", help="compare run JSON files of both backends")
    p.add_argument("runs", type=Path, nargs="+")
    p.add_argument("--min-runs", type=int, default=5)
    p.add_argument("--json", type=Path, help="also write the report as JSON")
    p.add_argument("--markdown", type=Path, help="also write the Markdown table")
    args = ap.parse_args(argv)
    try:
        if args.command == "run":
            result = asyncio.run(_record(args))
            args.output.write_text(json.dumps(result, indent=2) + "\n")
            print(json.dumps({"output": str(args.output), **result["metrics"]}, indent=2))
            return 0
        rep = report([json.loads(p.read_text()) for p in args.runs], min_runs=args.min_runs)
        text = format_report(rep)
        print(text, end="")
        if args.json:
            args.json.write_text(json.dumps(rep, indent=2) + "\n")
        if args.markdown:
            args.markdown.write_text(text)
        return 0 if rep["pass"] else 2
    except RunError as e:
        print(f"::error::{e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
