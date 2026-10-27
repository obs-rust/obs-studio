#!/usr/bin/env -S uv run --script
"""Summarize one sanitize.py result directory as Markdown.

    triage.py <out>/<tool> [--tool=asan|tsan|valgrind] [--fail-on-findings]

The tool defaults to the directory name (sanitize.py writes to <out>/<tool>).

A report counts as an OBS finding when OBS's own sources (libobs/, frontend/,
plugins/, shared/, deps/, test/) are where it happens, not merely somewhere up
the stack:

- ASan/UBSan: every report with an OBS frame (these sanitizers have no false
  positives from uninstrumented code that matter here).
- TSan: both racing accesses have an OBS frame in their top two frames. Races
  inside uninstrumented Qt, GLib, D-Bus, Mesa or PulseAudio are listed
  separately as external; TSan cannot see those libraries' locking.
- valgrind: memory errors with an OBS frame in the top four frames; definite
  leaks through OBS code only when the process exited without crashing (after a
  crash, live allocations show up as "definitely lost").

Unit-test failures that are limitations of the tool itself (KNOWN) are listed
but not counted.

Exit status is 0, or 1 with --fail-on-findings when there is a finding.
"""

import re
import sys
from pathlib import Path

OBS_FRAME = re.compile(r"(?<![\w/])(?:\.\./)*(?:[\w.-]+/)*?((?:libobs|frontend|plugins|shared|deps|test)/[\w./+-]+:\d+)")
# (tool, test) -> why its failure under that tool is not an OBS bug.
KNOWN = {
    ("valgrind", "test_pipe"): "valgrind runs posix_spawn as a plain fork, so a missing binary's exec "
    "failure is never reported and test_pipe_create2_missing_binary gets a pipe back",
    ("valgrind", "test_svt_av1"): "SVT-AV1's encoder functions exhaust valgrind's VEX translation storage "
    '("VEX temporary storage exhausted")',
}
EXTERNAL = re.compile(r"/nix/store/|/usr/(?:include|lib)/")


def obs_frame(line):
    """The OBS source location in one stack-frame line, or None."""
    if EXTERNAL.search(line):
        return None
    m = OBS_FRAME.search(line)
    return m.group(1) if m else None


def frames(lines):
    return [line for line in lines if re.match(r"\s+(#\d+ |at 0x|by 0x)", line)]


def first_obs(lines, depth):
    for line in frames(lines)[:depth]:
        loc = obs_frame(line)
        if loc:
            return loc
    return None


def function_at(lines, loc):
    """The function name on the frame line that contains loc."""
    for line in frames(lines):
        if loc in line:
            m = re.search(r"(?:in |0x[0-9A-Fa-f]+: |#\d+ )([^(\s][^(]*?)\s*(?:\(|/|\.\./|$)", line)
            return m.group(1).strip() if m else ""
    return ""


def sanitizer_reports(path):
    """ASan/UBSan/TSan reports in one log file, as lists of lines."""
    text = path.read_text(errors="replace")
    if "ThreadSanitizer" in text:
        parts = re.split(r"^={18}\n|^(?===\d+==ERROR: ThreadSanitizer)", text, flags=re.M)
        return [p.splitlines() for p in parts if "WARNING: ThreadSanitizer" in p or "ERROR: ThreadSanitizer" in p]
    reports = []
    for chunk in re.split(r"^(?==+\d+==ERROR|.*runtime error:)", text, flags=re.M):
        if "ERROR: AddressSanitizer" in chunk or "ERROR: LeakSanitizer" in chunk or "runtime error:" in chunk:
            reports += leaks(chunk.splitlines())
    return reports


def leaks(lines):
    """A LeakSanitizer report split into one report per leak (header kept)."""
    if not any("ERROR: LeakSanitizer" in line for line in lines):
        return [lines]
    starts = [i for i, line in enumerate(lines) if re.match(r"(Direct|Indirect) leak of", line)]
    if not starts:
        return [lines]
    head = lines[:starts[0]]
    return [head + lines[a:b] for a, b in zip(starts, starts[1:] + [len(lines)])]


def unloaded_init_leak(lines):
    """A leak allocated in a library's one-time init that was unloaded before
    LeakSanitizer ran (the OpenGL driver under gs_create): every frame between
    the allocator and OBS's first is `<unknown module>` or pthread_once."""
    if not any("ERROR: LeakSanitizer" in line for line in lines):
        return False
    above = []
    for line in frames(lines)[1:]:
        if obs_frame(line):
            break
        above.append(line)
    else:
        return False
    return any("<unknown module>" in line for line in above) and all(
        "<unknown module>" in line or "pthread_once" in line for line in above
    )


def asan_finding(lines):
    head = next((line for line in lines if "ERROR:" in line or "runtime error:" in line), lines[0])
    if "runtime error:" in head:
        m = re.search(r"((?:libobs|frontend|plugins|shared|deps|test)/[\w./+-]+:\d+):\d+: runtime error: (.*)", head)
        if m:
            return f"UBSan: {m.group(2)}", m.group(1)
    kind = re.sub(r"^.*ERROR: (\w+Sanitizer): ", r"\1: ", head).split(" on ")[0].split(" (pc")[0].strip()
    loc = first_obs(lines, 40)
    return kind, loc


def tsan_finding(lines):
    """(kind, signature, is_obs) for one TSan report."""
    if any("ERROR: ThreadSanitizer" in line for line in lines):
        kind, loc = asan_finding(lines)
        return kind, (loc,), loc is not None
    kind = re.search(r"WARNING: ThreadSanitizer: ([^(\n]+)", "\n".join(lines)).group(1).strip()
    blocks, cur = [], []
    for line in lines:
        if re.match(r"\s{2}\S", line) and not re.match(r"\s+#\d", line):
            if cur:
                blocks.append(cur)
            cur = [line]
        elif cur:
            cur.append(line)
    if cur:
        blocks.append(cur)
    access = [b for b in blocks if re.match(r"\s+(Write|Read|Previous|Atomic)", b[0])]
    if kind.startswith("data race") and len(access) >= 2:
        locs = [first_obs(b[1:], 2) for b in access[:2]]
        return kind, tuple(locs), all(locs)
    first = blocks[0][1:] if blocks else lines
    loc = first_obs(first, 2)
    return kind, (loc,), loc is not None


def valgrind_findings(path):  # noqa: C901
    text = path.read_text(errors="replace")
    pids = re.findall(r"^==(\d+)==", text, flags=re.M)
    if not pids:
        return [], None
    pid = max(set(pids), key=pids.count)
    lines = [line.split("== ", 1)[1] if "== " in line else "" for line in text.splitlines() if line.startswith(f"=={pid}==")]
    records, cur = [], []
    for line in lines + [""]:
        if line.strip():
            cur.append(line)
        elif cur:
            records.append(cur)
            cur = []
    crash = None
    out = []
    for rec in records:
        head = rec[0]
        if head.startswith("Process terminating"):
            crash = (head, first_obs(rec, 6) or (frames(rec)[0].strip() if frames(rec) else ""))
            continue
        if re.match(r"(Invalid|Conditional jump|Use of uninitialised|Syscall param|Mismatched|Source and destination|Argument)", head):
            loc = first_obs(rec, 4)
            if loc:
                out.append((head.split(" at 0x")[0], loc, rec))
    if crash is None:
        for rec in records:
            if "definitely lost in loss record" in rec[0]:
                loc = first_obs(rec, 12)
                if loc:
                    out.append(("definite leak", loc, rec))
    return out, crash


def section(title, items, limit=12):
    md = [f"#### {title}", ""]
    if not items:
        return md + ["None.", ""]
    for i, (n, label, detail) in enumerate(items):
        if i == limit:
            md.append(f"- ...and {len(items) - limit} more (see the uploaded logs)")
            break
        md.append(f"- {n}× **{label}**" + (f" at `{detail}`" if detail else ""))
    return md + [""]


def group(entries):
    """[(label, detail)] -> [(count, label, detail)] in first-seen order."""
    counts = {}
    for key in entries:
        counts[key] = counts.get(key, 0) + 1
    return [(n, label, detail) for (label, detail), n in counts.items()]


def main():  # noqa: C901
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 1:
        sys.exit(__doc__)
    out = Path(args[0])
    tool = next((a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--tool=")), out.name)
    md = [f"### {tool}", ""]
    findings = 0

    summary = out / "tests" / "summary.txt"
    if summary.exists():
        rows = summary.read_text().split("\n")
        ran = [r for r in rows if " exit=" in r]
        failed = [r for r in ran if not r.endswith(" exit=0") and (tool, r.split()[0]) not in KNOWN]
        known = [r.split()[0] for r in ran if not r.endswith(" exit=0") and (tool, r.split()[0]) in KNOWN]
        skipped = [r.split()[0] for r in rows if "skipped" in r]
        md.append(f"**Unit tests:** {len(ran) - len(failed) - len(known)}/{len(ran)} clean"
                  + (f", {len(known)} known tool limitations" if known else "")
                  + (f", skipped: {', '.join(skipped)}" if skipped else ""))
        md.append("")
        findings += len(failed)
        md += section("Unit tests with a non-zero exit", [(1, r.split()[0], r.split("=")[-1]) for r in failed])
        if known:
            md += ["#### Known tool limitations (not counted)", ""] + [f"- **{n}**: {KNOWN[(tool, n)]}" for n in known] + [""]
        test_reports, test_ext = [], []
        test_logs = sorted((out / "tests").glob("*.*san.*"))
        test_logs += [f for f in sorted((out / "tests").glob("*.log")) if not f.name.endswith(".vg.log")]
        for f in test_logs:
            for rep in sanitizer_reports(f):
                if tool == "asan":
                    kind, loc = asan_finding(rep)
                    external = unloaded_init_leak(rep)
                else:
                    # No OBS frame on any side: uninstrumented glib, Mesa.
                    kind, loc, _ = tsan_finding(rep)
                    external = not any(loc)
                (test_ext if external else test_reports).append((f"{f.name.split('.')[0]}: {kind}", str(loc)))
        for f in sorted((out / "tests").glob("*.vg.log")):
            if (tool, f.name.split(".")[0]) in KNOWN:
                continue
            errs, crash = valgrind_findings(f)
            test_reports += [(f"{f.name.split('.')[0]}: {k}", loc) for k, loc, _ in errs]
        findings += len(test_reports)
        md += section("Sanitizer reports in unit tests", group(test_reports))
        if test_ext:
            title = "leaks in unloaded library init" if tool == "asan" else "no OBS frame in the report"
            md += section(f"External: {title} (not counted)", group(test_ext), limit=8)
    else:
        md += ["**Unit tests:** not run", ""]

    exit_file = out / "app-exit.txt"
    launch = exit_file.read_text().strip() if exit_file.exists() else "not run"
    md.append(
        f"**App launch:** `{launch}` (exited = shut down after SIGINT; "
        "killed = still running after the grace period, then SIGKILL)"
    )
    md.append("")
    hang = out / "hang-backtrace.txt"
    if "(killed)" in launch:
        # A shutdown hang is an OBS finding (#462).
        findings += 1
        if hang.exists():
            lines = hang.read_text(errors="replace").splitlines()
            md += ["<details><summary>Threads when it hung (gdb)</summary>", "", "```", *lines[:400], "```", "</details>", ""]
        else:
            md += ["No backtrace was captured (gdb missing or not allowed to attach).", ""]
    if tool == "valgrind":
        log = out / "app.vg.log"
        if log.exists():
            errs, crash = valgrind_findings(log)
            items = [(k, loc) for k, loc, _ in errs]
            if crash:
                md += [f"Process crashed: {crash[0]} at `{crash[1]}`. Leak results skipped (live allocations look lost after a crash).", ""]
                if OBS_FRAME.search(crash[1]):
                    items.insert(0, ("crash (" + re.sub(r".*signal \d+ \((\w+)\).*", r"\1", crash[0]) + ")", crash[1]))
            findings += len(items)
            md += section("OBS memory errors and leaks", group(items))
    else:
        obs, ext = [], []
        # Sanitizer log files first, then the app's stderr: UBSan in particular
        # can ignore log_path and print there. A report seen in both counts once.
        logs = sorted(out.glob(f"{'tsan' if tool == 'tsan' else '[au]bsan'}.*")) + [out / "app.log"]
        seen = set()
        for f in [f for f in logs if f.exists()]:
            for rep in sanitizer_reports(f):
                if tool == "tsan":
                    kind, sig, is_obs = tsan_finding(rep)
                    detail = " ↔ ".join(s or "external" for s in sig)
                else:
                    kind, loc = asan_finding(rep)
                    detail, is_obs = loc or "", loc is not None and not unloaded_init_leak(rep)
                if f.name == "app.log" and (kind, detail) in seen:
                    continue
                seen.add((kind, detail))
                (obs if is_obs else ext).append((kind, detail))
        findings += len(obs)
        md += section("OBS findings", group(obs))
        if ext:
            md += section("External / uninstrumented (not counted)", group(ext), limit=8)

    print("\n".join(md))
    if findings and "--fail-on-findings" in sys.argv:
        sys.exit(1)


if __name__ == "__main__":
    main()
