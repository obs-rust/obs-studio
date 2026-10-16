#!/usr/bin/env -S uv run --script
"""Local gate for obs-rust/obs-studio (zackees/ci.yml GATE-001..010).

One entry point for everything CI's lint and unit-test jobs check, split into
lanes that `ci-lint local-gate run` caches on their inputs and attests in the
commit message (see local-gate.toml and ci-attestations.yml).

    uv run ci/local_gate.py                 # every lane
    uv run ci/local_gate.py --lane lint     # one lane
    uv run ci/local_gate.py --lane lint --fix

Lanes:
  lint         clang-format 22.1.3 and gersemi 0.25.0 over the same files as
               build-aux/run-clang-format and run-gersemi, plus rustfmt,
               clippy -D warnings and the repo Dylints (dylints/*). Formatters are pinned through uvx, so no
               brew/zsh install is needed. Also ci/platform_ifdef_ratchet.py (#847).
  rust-test    soldr cargo test --workspace --locked (Tiers 1-3), plus a
               --no-run build of the tests for x86_64-pc-windows-gnu.

Cargo runs with SOLDR_RUSTC_WRAPPER=none (soldr's zccache has replayed stale
rustc output; #134). Set OBS_GATE_ZCCACHE=1 to keep the cache locally.
  tier2-linux  rust/tools/linux-validate/run.sh: libobs + cmocka with
               ENABLE_RUST_LIBOBS OFF and ON in Docker, exported-symbol diff.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLANG_FORMAT = "clang-format==22.1.3"
GERSEMI = "gersemi==0.25.0"

# build-aux/run-clang-format
CLANG_DIRS = ("libobs", "libobs-*", "frontend", "plugins", "deps", "shared", "test", "rust")
CLANG_EXTS = (".c", ".cpp", ".h", ".hpp", ".m", ".mm")
CLANG_EXCLUDE = (
    "*/obs-websocket/deps/*",
    "*/decklink/*/decklink-sdk/*",
    "*/mac-syphon/syphon-framework/*",
    "*/libdshowcapture/*",
)
# build-aux/run-gersemi
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


def run(cmd: list[str], **kw) -> None:
    """Run `cmd`, streaming its output, and print hints for known environment failures."""
    print("+", " ".join(cmd), flush=True)
    seen = []
    with subprocess.Popen(
        cmd, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace", **kw
    ) as proc:
        for line in proc.stdout:
            print(line, end="", flush=True)
            if "soldr" in line or "FFmpeg" in line or "ocker" in line:
                seen.append(line)
    for hint in hints_for("".join(seen)):
        print(hint, flush=True)
    if proc.returncode:
        raise subprocess.CalledProcessError(proc.returncode, cmd)


def soldr_cargo(args: list[str], extra_env: dict[str, str] | None = None) -> None:
    """Run `soldr cargo ...` with soldr's zccache rustc wrapper disabled.

    The wrapper has replayed stale rustc output (old diagnostics, and green
    results that never compiled the current source; obs-rust/obs-studio#134),
    and the gate's attestation lets CI skip jobs on the strength of these
    lanes. OBS_GATE_ZCCACHE=1 keeps the cached build for local iteration.
    """
    env = dict(os.environ)
    if env.get("OBS_GATE_ZCCACHE") != "1":
        env["SOLDR_RUSTC_WRAPPER"] = "none"
    env.update(extra_env or {})
    run(["soldr", "cargo", *args], env=env)


def toolchain_gcc_ld() -> str:
    """The pinned toolchain's lld wrappers (rustlib/<host>/bin/gcc-ld)."""
    def rustc(*a: str) -> str:
        return subprocess.run(["soldr", "rustc", *a], cwd=ROOT, check=True, capture_output=True, text=True).stdout

    sysroot = rustc("--print", "sysroot").strip().splitlines()[-1]
    host = next(line.split()[1] for line in rustc("-vV").splitlines() if line.startswith("host:"))
    return os.path.join(sysroot, "lib", "rustlib", host, "bin", "gcc-ld")


# ci/ tests that need soldr on PATH (they skip without it).
SOLDR_SELF_TESTS = "test_local_gate_probe.py"


def self_test() -> None:
    """The gate's and the repo layout's unit tests: every ci/test_*.py.

    CI runs them in the Checks job (check-format.yaml) and sets
    OBS_GATE_SKIP_SELF_TEST=1 for the lint step on the scarce ubuntu-26.04
    runner (#427); locally they always run."""
    if os.environ.get("OBS_GATE_SKIP_SELF_TEST") == "1":
        # Only the tests that need this runner's soldr; the rest run in ci-self-test.
        print("+ ci/ self-tests: soldr-dependent only (OBS_GATE_SKIP_SELF_TEST=1; the Checks job runs the rest)", flush=True)
        run([sys.executable, "-m", "unittest", "discover", "-s", "ci", "-p", SOLDR_SELF_TESTS])
        return
    run([sys.executable, "-m", "unittest", "discover", "-s", "ci", "-p", "test_*.py"])


def platform_ifdef_ratchet() -> None:
    """Shared C/C++ gains no platform conditionals (#847); see ci/platform_ifdef_ratchet.py."""
    run([sys.executable, "ci/platform_ifdef_ratchet.py"])


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


def lane_lint(fix: bool) -> None:
    files = tracked_files()
    clang_format(files, fix)
    gersemi(files, fix)
    self_test()
    platform_ifdef_ratchet()
    soldr_cargo(["fmt", "--all"] + ([] if fix else ["--check"]))
    soldr_cargo(["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"])
    # Pre-expansion Dylints (dylints/*), so host-cfg'd code is checked on
    # one Linux lane (obs-rust/obs-studio#323). They scan whole src/ files,
    # inline test modules included, so lib + bins covers their scope;
    # --all-targets would also type-check integration tests that need
    # CARGO_BIN_EXE_* from a real build.
    soldr_cargo(["dylint", "--all", "--", "--workspace", "--lib", "--bins"])


# Every shipped Rust target, linted from this host (#472): soldr provides
# the Windows SDK (xwin) and the macOS SDK for cross-compiling.
CROSS_TARGETS = ("x86_64-pc-windows-msvc", "aarch64-apple-darwin", "x86_64-apple-darwin")


def lane_cross(fix: bool) -> None:
    # obs-c-oracle is a dev-only crate of C built for the host's tests; it
    # never ships and its C doesn't cross-compile to every target.
    scope = ["--workspace", "--exclude", "obs-c-oracle", "--lib", "--bins"]
    for target in CROSS_TARGETS:
        run(["soldr", "rustup", "target", "add", target])
        soldr_cargo(["clippy", *scope, "--locked", "--target", target, "--", "-D", "warnings"])
        soldr_cargo(["dylint", "--all", "--", *scope, "--target", target])
        # Every test binary, C oracle included, must also compile and link
        # for the target (#516, #695): cc compiles all the oracle's C, and
        # soldr's self-contained lld-link/ld64.lld links it on Linux.
        soldr_cargo(["test", "--workspace", "--no-run", "--locked", "--target", target])


def lane_rust_test(fix: bool) -> None:
    soldr_cargo(["test", "--workspace", "--locked"])
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
    soldr_cargo(["test", *pkgs, "--locked"])


def lane_tier2_linux(fix: bool) -> None:
    run(["bash", "rust/tools/linux-validate/run.sh"])


LANES = {"lint": lane_lint, "rust-test": lane_rust_test, "cross": lane_cross, "tier2-linux": lane_tier2_linux}

# The lint lane's formatter checks on their own, for CI's format jobs: the
# same pinned tools and files as `./lint`, so the two cannot disagree (#427).
CHECKS = {"clang-format": clang_format, "gersemi": gersemi}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lane", choices=sorted(LANES), action="append", help="run only this lane (repeatable)")
    parser.add_argument("--fix", action="store_true", help="lint: rewrite files instead of checking")
    parser.add_argument("--test-diff", metavar="BASE", help="run unit tests only for crates changed since BASE (fast gate)")
    parser.add_argument("--only", choices=sorted(CHECKS), help="run only this formatter check (CI format jobs)")
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
    for name in args.lane or list(LANES):
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
