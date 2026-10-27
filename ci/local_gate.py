#!/usr/bin/env -S uv run --script
"""Local gate for obs-rust/obs-studio (zackees/ci.yml GATE-001..010).

One entry point for everything CI's lint and unit-test jobs check, split into
lanes that `ci-lint local-gate run` caches on their inputs and attests in the
commit message (see local-gate.toml and ci-attestations.yml).

    uv run ci/local_gate.py                 # every lane
    uv run ci/local_gate.py --lane lint     # one lane
    uv run ci/local_gate.py --lane lint --fix

Lanes:
  lint         clang-format 22.1.3 and gersemi 0.25.0 over the C/C++ and
               CMake trees, plus rustfmt,
               clippy -D warnings and the repo Dylints (dylints/*). Formatters are pinned through uvx, so no
               brew/zsh install is needed. Also ci/platform_ifdef_ratchet.py (#847)
               and ci/perf_guard_lint.py (#1303). Plus one clippy per Cargo
               feature set in FEATURE_SETS (#2056), and a check that every
               workspace feature is built on and off by some clippy run.
  rust-test    soldr cargo test --workspace --locked (Tiers 1-3), the
               FEATURE_TEST_SETS runs, plus a --no-run build of the tests
               for x86_64-pc-windows-gnu.

Cargo runs with SOLDR_RUSTC_WRAPPER=none (soldr's zccache has replayed stale
rustc output; #134). Set OBS_GATE_ZCCACHE=1 to keep the cache locally.
  tier2-linux  rust/tools/linux-validate/run.py: libobs + cmocka with
               libobs and libobs-c-reference in Docker, exported-symbol diff.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from nextest_tool import ensure_nextest

ROOT = Path(__file__).resolve().parent.parent
CLANG_FORMAT = "clang-format==22.1.3"
GERSEMI = "gersemi==0.25.0"
RUFF = "ruff==0.14.0"

# clang-format source sets (formerly build-aux/run-clang-format, #2708)
CLANG_DIRS = ("libobs", "libobs-*", "frontend", "plugins", "deps", "shared", "test", "rust")
CLANG_EXTS = (".c", ".cpp", ".h", ".hpp", ".m", ".mm")
CLANG_EXCLUDE = (
    "*/obs-websocket/deps/*",
    "*/decklink/*/decklink-sdk/*",
    "*/mac-syphon/syphon-framework/*",
    "*/libdshowcapture/*",
)
# gersemi source sets (formerly build-aux/run-gersemi, #2708)
GERSEMI_DIRS = ("libobs", "libobs-*", "frontend", "plugins", "deps", "shared", "cmake", "test")
GERSEMI_EXCLUDE = (
    "*/jansson/*",
    "*/decklink/*/decklink-sdk/*",
    "*/obs-websocket/*",
    "*/obs-browser/*",
    "*/libdshowcapture/*",
    "cmake/Modules/*",
    "*/legacy.cmake",
)


TROUBLESHOOTING = "docs/rust-port/troubleshooting.md"
# Environment failures that look like code failures (#694): output pattern ->
# a one-line hint. Each one has a section in TROUBLESHOOTING.
HINTS = (
    (
        re.compile(r"soldr: .*Permission denied"),
        "soldr cannot write its cache or target directory; fix the ownership of ~/.soldr and target/",
    ),
    (
        re.compile(r"soldr: (command )?not found|soldr: No such file"),
        "soldr is not on PATH; install it with `uv tool install --upgrade soldr`",
    ),
    (re.compile(r"soldr: .*disk space is low"), "less than 10 GiB free; run `soldr gc` or free disk space"),
    (
        re.compile(r"Could NOT find FFmpeg|FFmpeg.*Required is at least version"),
        "Tier 2 found the host's FFmpeg; run it in Docker with ./unittest instead of against host packages",
    ),
    (
        re.compile(r"Cannot connect to the Docker daemon|docker: (command )?not found|permission denied.*docker\.sock"),
        "Tier 2 needs a running Docker you can use without sudo",
    ),
)


def hints_for(text: str) -> list[str]:
    """One hint per known environment failure in `text`, in HINTS order."""
    return [
        f"hint: {hint} ({TROUBLESHOOTING}); or open the PR and let CI run."
        for pattern, hint in HINTS
        if pattern.search(text)
    ]


def run(cmd: list[str], buffer: list[str] | None = None, **kw) -> None:
    """Run `cmd`, streaming its output, and print hints for known environment failures.

    With `buffer`, the output (command line and hints included) is collected
    there instead of printed, for a step that runs beside another one.
    """
    def emit(text: str) -> None:
        if buffer is None:
            print(text, end="", flush=True)
        else:
            buffer.append(text)

    emit("+ " + " ".join(cmd) + "\n")
    seen = []
    with subprocess.Popen(
        cmd, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace", **kw
    ) as proc:
        for line in proc.stdout:
            emit(line)
            if "soldr" in line or "FFmpeg" in line or "ocker" in line:
                seen.append(line)
    for hint in hints_for("".join(seen)):
        emit(hint + "\n")
    if proc.returncode:
        raise subprocess.CalledProcessError(proc.returncode, cmd)


def soldr_cargo(args: list[str], extra_env: dict[str, str] | None = None) -> None:
    """Run `soldr cargo ...` with soldr's zccache rustc wrapper disabled.

    The wrapper has replayed stale rustc output (old diagnostics, and green
    results that never compiled the current source; obs-rust/obs-studio#134),
    and the gate's attestation lets CI skip jobs on the strength of these
    lanes. OBS_GATE_ZCCACHE=1 keeps the cached build for local iteration.
    """
    run(["soldr", "cargo", *args], env=soldr_env(extra_env))


def soldr_env(extra_env: dict[str, str] | None = None) -> dict[str, str]:
    """The environment of every soldr step: the wrapper off (see soldr_cargo)."""
    env = dict(os.environ)
    if env.get("OBS_GATE_ZCCACHE") != "1":
        env["SOLDR_RUSTC_WRAPPER"] = "none"
    env.update(extra_env or {})
    return env


def soldr_dylint(args: list[str], buffer: list[str] | None = None, beside_clippy: bool = False) -> None:
    """Run the repo Dylints through soldr's native `soldr dylint` (#2293).

    `soldr cargo dylint` defers to whatever cargo-dylint is on PATH, which
    skips soldr's managed driver and its verdict ledger (soldr#3894): a
    Dylint pass whose exact inputs already passed is replayed.
    """
    env = {
        # soldr's managed driver even where a cargo-dylint is on PATH (it
        # defers to that one otherwise; CI runners have none).
        "SOLDR_FORCE_MANAGED_CARGO_SUBCOMMANDS": "1",
    }
    if beside_clippy:
        # Its own target root: soldr's no-cache preflight refuses to touch a
        # target dir while the Clippy that runs beside it holds a .cargo-lock.
        # Run alone, Dylint shares target/ and its CI cache instead.
        env["CARGO_TARGET_DIR"] = str(ROOT / "target-dylint")
    run(["soldr", "dylint", *args], buffer=buffer, env=soldr_env(env))


def toolchain_gcc_ld() -> str:
    """The pinned toolchain's lld wrappers (rustlib/<host>/bin/gcc-ld)."""
    def rustc(*a: str) -> str:
        return subprocess.run(["soldr", "rustc", *a], cwd=ROOT, check=True, capture_output=True, text=True).stdout

    sysroot = rustc("--print", "sysroot").strip().splitlines()[-1]
    host = next(line.split()[1] for line in rustc("-vV").splitlines() if line.startswith("host:"))
    return os.path.join(sysroot, "lib", "rustlib", host, "bin", "gcc-ld")


# ci/ tests that need soldr on PATH (they skip without it).
SOLDR_SELF_TESTS = "test_local_gate_probe.py"

SELF_TEST_MAX_WORKERS = 8
SELF_TEST_TIMEOUT = 600


def self_test_workers() -> int:
    """CPUs this process may use (cgroup and affinity aware), at most 8."""
    usable = getattr(os, "process_cpu_count", os.cpu_count)() or 1
    return max(1, min(SELF_TEST_MAX_WORKERS, usable))


def self_test() -> None:  # noqa: C901
    """The gate's and the repo layout's unit tests: every ci/test_*.py.

    CI runs them in the Checks job (check-format.yaml) and sets
    OBS_GATE_SKIP_SELF_TEST=1 for the lint step on the scarce ubuntu-26.04
    runner (#427); locally they always run.

    One process per file, run in parallel: run serially they took about
    164 s of ./lint (#949). Each file's result prints as it finishes;
    failures are repeated with their full output at the end.
    """
    if os.environ.get("OBS_GATE_SKIP_SELF_TEST") == "1":
        # Only the tests that need this runner's soldr; the rest run in ci-self-test.
        print("+ ci/ self-tests: soldr-dependent only (OBS_GATE_SKIP_SELF_TEST=1; the Checks job runs the rest)", flush=True)
        run([sys.executable, "-m", "unittest", "discover", "-s", "ci", "-p", SOLDR_SELF_TESTS])
        return
    files = sorted((ROOT / "ci").glob("test_*.py"))

    def one(path: Path) -> tuple[str, int, str, float]:
        start = time.monotonic()
        try:
            result = subprocess.run(
                [sys.executable, "-W", "default::ResourceWarning", "-m", "unittest", "discover", "-s", "ci", "-p", path.name],
                cwd=ROOT,
                capture_output=True,
                text=True,
                errors="replace",
                timeout=SELF_TEST_TIMEOUT,
            )
        except subprocess.TimeoutExpired as e:
            output = (e.stdout or "") + (e.stderr or "")
            if isinstance(output, bytes):
                output = output.decode(errors="replace")
            return path.name, 1, f"{output}\ntimed out after {SELF_TEST_TIMEOUT} s", time.monotonic() - start
        # 5: the file has no tests (as the old single discover allowed).
        code = 0 if result.returncode == 5 else result.returncode
        output = result.stdout + result.stderr
        # A leaked file or response warns from a destructor, which no -W error
        # can turn into a failure; 781 such lines buried the Checks log (#2235).
        if code == 0 and "ResourceWarning" in output:
            code = 1
            output += "\nResourceWarning: close every file and response the test opens (#2235)"
        return path.name, code, output, time.monotonic() - start

    workers = self_test_workers()
    print(f"+ unittest: {len(files)} ci/test_*.py files, {workers} at a time", flush=True)
    failed = []
    with ThreadPoolExecutor(max_workers=workers) as pool:
        for future in as_completed([pool.submit(one, path) for path in files]):
            name, code, output, secs = future.result()
            print(f"  {'ok  ' if code == 0 else 'FAIL'} {secs:6.1f}s {name}", flush=True)
            if code:
                failed.append((name, output))
    for name, output in failed:
        print(f"==== {name}", flush=True)
        print(output, flush=True)
        for hint in hints_for(output):
            print(hint, flush=True)
    if failed:
        raise subprocess.CalledProcessError(1, ["unittest", *(name for name, _ in failed)])


def platform_ifdef_ratchet() -> None:
    """Shared C/C++ gains no platform conditionals (#847); see ci/platform_ifdef_ratchet.py."""
    run([sys.executable, "ci/platform_ifdef_ratchet.py"])


def perf_guard_lint() -> None:
    """Measured perf fixes keep their code shapes (#1303); see ci/perf-guards.toml."""
    run([sys.executable, "ci/perf_guard_lint.py"])


def tracked_files() -> list[str]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True).stdout
    return [p for p in out.decode().split("\0") if p and (ROOT / p).is_file()]


def top_dir_matches(path: str, dirs: tuple[str, ...]) -> bool:
    top = path.split("/", 1)[0]
    return "/" in path and any(fnmatch.fnmatch(top, d) for d in dirs)


def excluded(path: str, patterns: tuple[str, ...]) -> bool:
    return any(fnmatch.fnmatch(path, p) or fnmatch.fnmatch("/" + path, p) for p in patterns)


def uvx_tool(spec: str, binary: str) -> str:
    """Absolute path of `binary` from the pinned uvx environment for `spec`."""
    code = f"import shutil; print(shutil.which({binary!r}))"
    out = subprocess.run(
        ["uvx", "--from", spec, "python", "-c", code], check=True, capture_output=True, text=True
    ).stdout.strip()
    if not out or out == "None":
        sys.exit(f"local_gate: {binary} not found in uvx environment {spec}")
    return out


def batches(items: list[str], size: int) -> list[list[str]]:
    return [items[i : i + size] for i in range(0, len(items), size)]


def clang_format(files: list[str], fix: bool) -> None:
    sources = [
        f
        for f in files
        if top_dir_matches(f, CLANG_DIRS) and f.endswith(CLANG_EXTS) and not excluded(f, CLANG_EXCLUDE)
    ]
    exe = uvx_tool(CLANG_FORMAT, "clang-format")
    args = ["-i"] if fix else ["--dry-run", "--Werror"]
    print(f"+ clang-format {' '.join(args)} <{len(sources)} files>", flush=True)

    def one(chunk: list[str]) -> subprocess.CompletedProcess:
        return subprocess.run([exe, "-style=file", "-fallback-style=none", *args, *chunk], cwd=ROOT, capture_output=True, text=True)

    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
        results = list(pool.map(one, batches(sources, 50)))
    failed = [r for r in results if r.returncode != 0]
    for r in failed:
        sys.stderr.write(r.stderr)
    if failed:
        sys.exit("local_gate: clang-format check failed (fix with: uv run ci/local_gate.py --lane lint --fix)")


def gersemi(files: list[str], fix: bool) -> None:
    sources = [
        f
        for f in files
        if (f == "CMakeLists.txt" or top_dir_matches(f, GERSEMI_DIRS))
        and (f.endswith("CMakeLists.txt") or f.endswith(".cmake"))
        and not excluded(f, GERSEMI_EXCLUDE)
    ]
    exe = uvx_tool(GERSEMI, "gersemi")
    print(f"+ gersemi {'-i' if fix else '-c'} <{len(sources)} files>", flush=True)
    subprocess.run([exe, "-i" if fix else "-c", "--no-cache", *sources], cwd=ROOT, check=True)


def ruff_ratchet(files: list[str]) -> None:
    """The Python complexity ratchet (ci-lint PY-004, #962): ruff.toml
    selects only C901 and RUF100, so a new function over the ceiling fails
    and a `# noqa: C901` that no longer suppresses anything fails too."""
    sources = [f for f in files if f.endswith(".py")]
    exe = uvx_tool(RUFF, "ruff")
    print(f"+ ruff check (C901, RUF100) <{len(sources)} files>", flush=True)
    subprocess.run([exe, "check", "--no-cache", "--quiet", *sources], cwd=ROOT, check=True)


# A change to these can reformat files the change did not touch.
FORMAT_CONFIG_NAMES = (".clang-format", ".gersemirc")
FORMAT_CONFIG_PATHS = ("ci/local_gate.py",)  # pins CLANG_FORMAT and GERSEMI


def format_scope(files: list[str], changed: list[str] | None, ci: bool) -> list[str]:
    """Files clang-format and gersemi check (#1050).

    CI checks every file: on a PR this lane is the only format check. Locally
    only the files changed since origin/master: the rest is already formatted
    on master, where every push checks every file. Every file again when the
    changes are unknown or touch the formatter configuration.
    """
    if ci or changed is None:
        return files
    if any(Path(c).name in FORMAT_CONFIG_NAMES or c in FORMAT_CONFIG_PATHS for c in changed):
        return files
    wanted = set(changed)
    return [f for f in files if f in wanted]


def changed_since_master() -> list[str] | None:
    """Committed, staged, unstaged and untracked changes since the merge-base
    with origin/master, plus formatter configuration changed on master since
    then; None when there is no merge-base."""
    def git(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)

    base = git("merge-base", "HEAD", "origin/master")
    sha = (base.stdout or "").strip()
    if base.returncode or not sha:
        return None
    names: set[str] = set()
    for args in (
        ("diff", "--name-only", sha),
        ("ls-files", "--others", "--exclude-standard"),
        # Formatter configuration master changed after the branch point: an
        # unrebased branch must not pass against the old rules.
        ("diff", "--name-only", sha, "origin/master", "--", *FORMAT_CONFIG_PATHS,
         *(f":(glob)**/{n}" for n in FORMAT_CONFIG_NAMES)),
    ):
        out = git(*args)
        if out.returncode:
            return None
        names.update(line for line in (out.stdout or "").splitlines() if line)
    return sorted(names)


def check_test_binary_inventory() -> None:
    """Enforce Cargo's inventory and cross-check the toolchain-free linter."""
    import json
    from test_binary_cap import cargo_test_targets, category_violations, workspace_test_targets

    result = subprocess.run(
        ["soldr", "cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    payload = "\n".join(line for line in result.stdout.splitlines() if not line.startswith("#"))
    targets = cargo_test_targets(json.loads(payload))
    errors = category_violations(targets)
    if targets != workspace_test_targets():
        errors.append("manifest test discovery differs from Cargo metadata")
    if errors:
        raise RuntimeError("\n".join(errors))
    print(f"+ Cargo inventory: {len(targets)} category test executables", flush=True)


# Cargo feature sets that the --workspace runs do not build (#2056): Cargo
# unifies features across one build, so `clippy --workspace` sees each crate
# only with the union of what its dependents enable.
# The lint lane runs one clippy per FEATURE_SETS entry, the rust-test lane one
# test per FEATURE_TEST_SETS entry. check_feature_sets() fails the lint lane
# when a workspace feature is not built both on and off by some clippy run.
FEATURE_SETS: tuple[tuple[str, ...], ...] = (
    # libobs without GIO: libobs/cmake/os-linux.cmake and os-freebsd.cmake
    # build libobs_rust with NO_DEFAULT_FEATURES and add gio only when Gio
    # is found.
    ("-p", "libobs-rust", "--no-default-features"),
    # obs-util's own test and bench targets with gio off (the run above
    # builds obs-util as a library only).
    ("-p", "obs-util"),
    # The native libobs contracts of obs-tests-app. Tier 2 and the Qt SDK
    # step build them for a few tests on macOS and Windows only.
    ("-p", "obs-studio-rust", "--features", "libobs-integration"),
)
FEATURE_TEST_SETS: tuple[tuple[str, ...], ...] = (
    ("-p", "obs-util"),
)
_NEEDS_RUST_ARCHIVE = (
    "obs-sys/build.rs needs OBS_BUILD_RUST_ARCHIVE (a release libobs-rust staticlib); "
    "Tier 2 checks it after its native build (rust/tools/tier2-validate/validate.sh, validate.ps1)"
)
FEATURE_SET_EXEMPT: dict[tuple[str, str], str] = {
    ("obs-studio-rust", "bundled-native"): _NEEDS_RUST_ARCHIVE,
    ("obs-sys", "bundled"): _NEEDS_RUST_ARCHIVE,
}


def workspace_manifests() -> dict[str, dict]:
    """Package name -> parsed Cargo.toml of every workspace member."""
    import tomllib

    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]
    excluded = {(ROOT / p).resolve() for p in workspace.get("exclude", [])}
    manifests = {}
    for pattern in workspace["members"]:
        for path in sorted(ROOT.glob(f"{pattern}/Cargo.toml")):
            if path.parent.resolve() in excluded:
                continue
            data = tomllib.loads(path.read_text(encoding="utf-8"))
            manifests[data["package"]["name"]] = data
    return manifests


def _workspace_deps(manifest: dict, names: set[str]) -> dict[str, list[dict]]:
    """The workspace dependencies of one manifest, every kind and target:
    every run is --all-targets, so dev-dependencies are built too."""
    deps: dict[str, list[dict]] = {}
    for table in (manifest, *manifest.get("target", {}).values()):
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            for dep, spec in table.get(kind, {}).items():
                if dep in names and isinstance(spec, dict):
                    deps.setdefault(dep, []).append(spec)
    return deps


def _resolve(manifests: dict[str, dict], name: str, features: tuple[str, ...], default: bool) -> tuple[set[str], set[tuple[str, str]]]:  # noqa: C901
    """The crates one `-p name` run builds and the (crate, feature) pairs it
    enables, with Cargo's feature syntax (dep:x, x/y, x?/y)."""
    names = set(manifests)
    deps = {n: _workspace_deps(m, names) for n, m in manifests.items()}
    active: set[str] = set()
    on: set[tuple[str, str]] = set()
    weak: set[tuple[str, str]] = set()
    work: list[tuple[str, str | None]] = [(name, None), *((name, f) for f in features)]
    if default:
        work.append((name, "default"))

    def use(crate: str, spec: dict) -> None:
        work.append((crate, None))
        if spec.get("default-features", True):
            work.append((crate, "default"))
        work.extend((crate, f) for f in spec.get("features", []))

    while work:
        while work:
            crate, feature = work.pop()
            if feature is None:
                if crate not in active:
                    active.add(crate)
                    for dep, specs in deps[crate].items():
                        for spec in specs:
                            if not spec.get("optional"):
                                use(dep, spec)
                continue
            if (crate, feature) in on:
                continue
            on.add((crate, feature))
            table = manifests[crate].get("features", {})
            if feature not in table:
                # The implicit feature of an optional dependency.
                for spec in deps[crate].get(feature, []):
                    use(feature, spec)
                continue
            for item in table[feature]:
                if item.startswith("dep:"):
                    for spec in deps[crate].get(item[4:], []):
                        use(item[4:], spec)
                elif "/" in item:
                    dep, sub = item.split("/", 1)
                    if dep.endswith("?"):
                        weak.add((dep[:-1], sub))
                    elif dep in deps[crate]:
                        for spec in deps[crate][dep]:
                            use(dep, spec)
                        work.append((dep, sub))
                else:
                    work.append((crate, item))
        # `x?/y` enables y only where x is built for another reason.
        work = [(dep, sub) for dep, sub in weak if dep in active and (dep, sub) not in on]
    return active, {(c, f) for c, f in on if c in active and f != "default"}


def active_crates(manifests: dict[str, dict], name: str, features: tuple[str, ...], default: bool) -> set[str]:
    return _resolve(manifests, name, features, default)[0]


def enabled_features(manifests: dict[str, dict], name: str, features: tuple[str, ...], default: bool) -> set[tuple[str, str]]:
    return _resolve(manifests, name, features, default)[1]


def _parse_set(args: tuple[str, ...]) -> tuple[str, tuple[str, ...], bool]:
    name, features, default = "", [], True
    rest = iter(args)
    for arg in rest:
        if arg == "-p":
            name = next(rest)
        elif arg == "--no-default-features":
            default = False
        elif arg == "--features":
            features += next(rest).split(",")
        else:
            raise ValueError(f"feature set {' '.join(args)}: unsupported argument {arg}")
    return name, tuple(features), default


def feature_coverage(manifests: dict[str, dict], sets: tuple[tuple[str, ...], ...], exempt: dict[tuple[str, str], str]) -> list[str]:  # noqa: C901
    """Every feature of a workspace crate is built on in one clippy run and
    off in another (the --workspace run, where Cargo unifies features, or a
    FEATURE_SETS entry), unless `exempt` says why not. A run turns a
    feature off only where it builds the crate."""
    errors = []
    runs = []
    built: set[str] = set()
    unified: set[tuple[str, str]] = set()
    for name in manifests:
        active, on = _resolve(manifests, name, (), True)
        built |= active
        unified |= on
    runs.append((built, unified))
    for args in sets:
        name, features, default = _parse_set(args)
        label = " ".join(args)
        if name not in manifests:
            errors.append(f"{label}: {name} is not a workspace crate")
            continue
        unknown = [f for f in features if f not in manifests[name].get("features", {})]
        if unknown:
            errors.append(f"{label}: {name} has no feature {', '.join(unknown)}")
            continue
        runs.append(_resolve(manifests, name, features, default))
    for (crate, feature), why in sorted(exempt.items()):
        if feature not in manifests.get(crate, {}).get("features", {}):
            errors.append(f"{crate}/{feature}: exempt, but there is no such feature")
        elif not why.strip():
            errors.append(f"{crate}/{feature}: the exemption needs a reason")
    for crate in sorted(manifests):
        for feature in sorted(manifests[crate].get("features", {})):
            if feature == "default" or (crate, feature) in exempt:
                continue
            if not any((crate, feature) in on for _, on in runs):
                errors.append(f"{crate}/{feature}: no clippy run builds it on (add a FEATURE_SETS entry)")
            if not any(crate in active and (crate, feature) not in on for active, on in runs):
                errors.append(f"{crate}/{feature}: no clippy run builds it off (add a FEATURE_SETS entry)")
    return errors


def check_feature_sets() -> None:
    errors = feature_coverage(workspace_manifests(), FEATURE_SETS, FEATURE_SET_EXEMPT)
    if errors:
        raise RuntimeError("Cargo feature sets (#2056):\n" + "\n".join(errors))
    print(f"+ Cargo feature sets: {len(FEATURE_SETS)} runs, every feature built on and off", flush=True)


def lane_lint(fix: bool) -> None:
    files = tracked_files()
    in_ci = bool(os.environ.get("GITHUB_ACTIONS"))
    fmt_files = format_scope(files, None if in_ci else changed_since_master(), in_ci)
    if len(fmt_files) != len(files):
        print(f"+ formatting {len(fmt_files)} files changed since origin/master (CI checks all)", flush=True)
    clang_format(fmt_files, fix)
    gersemi(fmt_files, fix)
    ruff_ratchet(files)
    self_test()
    check_test_binary_inventory()
    check_feature_sets()
    platform_ifdef_ratchet()
    perf_guard_lint()
    soldr_cargo(["fmt", "--all"] + ([] if fix else ["--check"]))
    # Pre-expansion Dylints (dylints/*), so host-cfg'd code is checked on
    # one Linux lane (obs-rust/obs-studio#323). They scan whole src/ files,
    # inline test modules included, so lib + bins covers their scope;
    # --all-targets would also type-check integration tests that need
    # CARGO_BIN_EXE_* from a real build. The pass builds with the Dylint
    # nightly in its own target dir, so it runs beside the stable Clippy
    # steps instead of after them (#2293); its output prints once it ends.
    dylint_output: list[str] = []
    with ThreadPoolExecutor(max_workers=1) as pool:
        dylint = pool.submit(soldr_dylint, ["--all", "--", "--workspace", "--lib", "--bins"], dylint_output, beside_clippy=True)
        try:
            soldr_cargo(["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"])
            for args in FEATURE_SETS:
                soldr_cargo(["clippy", *args, "--all-targets", "--locked", "--", "-D", "warnings"])
        finally:
            dylint_error = dylint.exception()
            print("".join(dylint_output), end="", flush=True)
    if dylint_error is not None:
        raise dylint_error


# Cross targets linted from this host (#472, #955): soldr provides the
# Windows SDK (xwin) and the macOS SDK. The PR gate covers win-x64 and
# mac-arm64 here (linux-x64 is the host, linted by the lint lane); the release
# gate adds win-arm64, mac-x64 and linux-arm64.
CROSS_TARGETS = ("x86_64-pc-windows-msvc", "aarch64-apple-darwin")
RELEASE_CROSS_TARGETS = ("aarch64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu")


def target_std_installed(target: str) -> bool:
    """Whether the toolchain has `target`'s core library on disk."""
    sysroot = subprocess.run(
        ["soldr", "rustc", "--print", "sysroot"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()
    return any((Path(sysroot) / "lib" / "rustlib" / target / "lib").glob("libcore-*.rlib"))


def add_target(target: str) -> None:
    run(["soldr", "rustup", "target", "add", target])
    # A restored toolchain cache can keep rustup's record of a target's
    # rust-std without its files: `target add` then says it is up to date and
    # cargo fails with "can't find crate for `core`". Reinstall it then.
    if not target_std_installed(target):
        print(f"rust-std for {target} is recorded but missing; reinstalling it", flush=True)
        run(["soldr", "rustup", "target", "remove", target])
        run(["soldr", "rustup", "target", "add", target])


def cross_lint(targets: tuple[str, ...], link: bool) -> None:
    # obs-c-oracle is a dev-only crate of C built for the host's tests; it
    # never ships and its C doesn't cross-compile to every target.
    scope = ["--workspace", "--exclude", "obs-c-oracle", "--lib", "--bins"]
    for target in targets:
        add_target(target)
        soldr_cargo(["clippy", *scope, "--locked", "--target", target, "--", "-D", "warnings"])
        soldr_dylint(["--all", "--", *scope, "--target", target])
        if link:
            # Every test binary, C oracle included, must also compile and link
            # for the target (#516, #695): cc compiles all the oracle's C, and
            # soldr's self-contained lld-link/ld64.lld links it on Linux.
            soldr_cargo(["test", "--workspace", "--no-run", "--locked", "--target", target])


def lane_cross(fix: bool) -> None:
    cross_lint(CROSS_TARGETS, link=True)


def lane_cross_release(fix: bool) -> None:
    # Release gate only (#955): lints, not links, for the extra targets.
    cross_lint(RELEASE_CROSS_TARGETS, link=False)


def rust_tests(packages: list[str]) -> None:
    """Schedule unchanged category binaries globally; retain Cargo doctests."""
    executable = ensure_nextest()
    if executable is None:
        soldr_cargo(["test", *packages, "--locked"])
        return
    env = {"PATH": str(executable.parent) + os.pathsep + os.environ.get("PATH", "")}
    failure = None
    try:
        soldr_cargo(["nextest", "run", *packages, "--locked", "--no-fail-fast",
                     "--test-threads", str(min(4, os.cpu_count() or 1)),
                     "--profile", "default", "--config-file", str(Path(__file__).with_name("nextest.toml")),
                     "--user-config-file", "none", "--ignore-default-filter", "--retries", "0",
                     "--no-tests", "pass" if "-p" in packages else "fail"], env)
    except subprocess.CalledProcessError as error:
        failure = error
    soldr_cargo(["test", *packages, "--locked", "--doc"])
    if failure is not None:
        raise failure


def lane_rust_test(fix: bool) -> None:
    rust_tests(["--workspace"])
    for args in FEATURE_TEST_SETS:
        rust_tests(list(args))
    # Every test binary, C oracle included, must also link for a GCC-based
    # Windows target (obs-rust/obs-studio#291). soldr provides the
    # mingw-w64 cross toolchain; nothing runs, so no Wine is needed.
    # The target is added here, not in rust-toolchain.toml: listing it there
    # made soldr on a Windows host build and run every test as windows-gnu.
    run(["soldr", "rustup", "target", "add", "x86_64-pc-windows-gnu"])
    # Link through lld: GNU ld scans archives once, in order, and rustc can
    # place the oracle's rlib before the crates that call into it.
    gcc_ld = toolchain_gcc_ld()
    soldr_cargo(
        ["test", "--workspace", "--locked", "--no-run", "--target", "x86_64-pc-windows-gnu"],
        {"CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS": f"-C link-arg=-B{gcc_ld} -C link-arg=-fuse-ld=lld"},
    )


def test_diff(base: str) -> None:
    """Unit tests for the crates a PR touches (ci/rust_scope.py), the fast
    gate's PR-path test step (#992). The full lane runs on master."""
    import json

    from rust_scope import affected

    files = subprocess.run(["git", "diff", "--name-only", base, "HEAD"], cwd=ROOT, check=True, capture_output=True, text=True).stdout.split()
    meta = json.loads(
        subprocess.run(["soldr", "cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], cwd=ROOT, check=True, capture_output=True, text=True).stdout
    )
    crates = {
        p["name"]: (str(Path(p["manifest_path"]).parent.relative_to(ROOT)), {d["name"] for d in p["dependencies"]})
        for p in meta["packages"]
    }
    mode, names = affected(files, crates)
    print(f"== test scope: {mode} {names}", flush=True)
    if mode == "skip":
        return
    pkgs = ["--workspace"] if mode == "workspace" else [a for n in names for a in ("-p", n)]
    rust_tests(pkgs)


def lane_tier2_linux(fix: bool) -> None:
    run([sys.executable, "rust/tools/linux-validate/run.py"])


LANES = {"lint": lane_lint, "rust-test": lane_rust_test, "cross": lane_cross, "cross-release": lane_cross_release, "tier2-linux": lane_tier2_linux}

# The lint lane's formatter checks on their own, for CI's format jobs: the
# same pinned tools and files as `./lint`, so the two cannot disagree (#427).
CHECKS = {"clang-format": clang_format, "gersemi": gersemi, "self-test": lambda _files, _fix: self_test()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lane", choices=sorted(LANES), action="append", help="run only this lane (repeatable)")
    parser.add_argument("--fix", action="store_true", help="lint: rewrite files instead of checking")
    parser.add_argument("--test-diff", metavar="BASE", help="run unit tests only for crates changed since BASE (fast gate)")
    parser.add_argument("--only", choices=sorted(CHECKS), help="run only this check (CI format jobs)")
    args = parser.parse_args()
    if args.test_diff:
        try:
            test_diff(args.test_diff)
        except subprocess.CalledProcessError as e:
            print(f"== test-diff: FAILED ({e.returncode})", flush=True)
            return 1
        return 0
    if args.only:
        CHECKS[args.only](tracked_files(), args.fix)
        print(f"== {args.only}: ok", flush=True)
        return 0
    # cross-release runs only in the release gate (#955).
    for name in args.lane or [n for n in LANES if n != "cross-release"]:
        start = time.monotonic()
        print(f"== lane {name}", flush=True)
        try:
            LANES[name](args.fix)
        except subprocess.CalledProcessError as e:
            print(f"== lane {name}: FAILED ({e.returncode})", flush=True)
            return 1
        except FileNotFoundError as e:
            print(f"local_gate: {e.filename or e}: command not found", flush=True)
            if e.filename == "soldr":
                print(f"hint: soldr is not on PATH; install it with `uv tool install --upgrade soldr` ({TROUBLESHOOTING}); "
                      "or open the PR and let CI run.", flush=True)
            print(f"== lane {name}: FAILED (127)", flush=True)
            return 1
        print(f"== lane {name}: ok ({time.monotonic() - start:.1f}s)", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
