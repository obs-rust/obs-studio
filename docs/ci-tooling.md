# CI and build tooling: how not to drown the machines

Many agents and people work on this repo at once, on shared machines and a
shared pool of GitHub runners. Every unnecessary cold build, raw Docker run or
"just re-run CI" multiplies across all of them. This repo uses a set of
purpose-built tools so that work is cached, shared, run locally where possible,
and skipped in CI when it has already been proven. This document says what
each tool is for and what you should do.

## A proto-tool, still evolving

These tools are not separate utilities that happen to coexist. Together they
form one tool that isn't finished yet:
- soldr caches the build;
- setup-soldr brings that cache to CI;
- bosn bounds the local Docker load;
- act2 replays CI jobs locally;
- zackees/ci.yml sets the protocol;
- ci.toml (or, here, `local-gate.toml` and `ci/ci_scope.py`) decides what
  runs for which change;
- attestation lets CI skip what was already proven.

They are still changing. Interfaces move, gaps exist, and this repo doesn't use
every piece yet.

The goal is to evolve how OBS-Studio-Rust is built until its CI is fast, by
following that general protocol rather than inventing repo-specific shortcuts.
When something here is slow or awkward, prefer the fix that moves the repo
closer to the protocol: adopt the next piece, or report the gap upstream in the
tool's repo (zackees/soldr, zackees/setup-soldr, zackees/bosn, zackees/act2,
zackees/ci.yml). One-off workarounds in this repo are a last resort. Treat this
document as a snapshot. When a tool changes, update the guide in the same PR
that adopts the change.

Read it before you build, test or push. It complements
[AGENTS.md](../AGENTS.md), [testing-policy.md](rust-port/testing-policy.md)
and [coordination.md](rust-port/coordination.md); where they set a rule, they
win.

## The rules in one screen

1. **Build Rust only through `soldr`.** `soldr cargo …`, never bare `cargo`,
   `rustc`, `rustup` or `rustfmt`. Run it from the repository root (the cargo
   workspace), not from `rust/`. Python goes through `uv run` / `uvx`, never
   bare `python`.
2. **Prove changes locally, once, with the local gate**, then push the stamped
   commit unchanged. A valid stamp lets PR CI skip the Linux test jobs.
3. **Don't use GitHub as your test runner.** Don't push to "see if CI passes",
   don't re-run whole workflows to debug, and don't look for a label to
   force more CI: no label changes what runs, only the changed paths do.
4. **Don't hit Docker directly for long or parallel work.** Docker on the shared
   machines is meant to be managed by `bosn`; never `docker system prune`.
5. **The gate lock is shared per clone.** If the gate says another gate is
   running, wait and retry; don't kill other people's gates.
6. **Expensive work belongs to the release gate**, not to every PR: full OBS
   app builds, Flatpak, and macOS/Windows packaging run nightly and on release.

## The tools

### soldr: the Rust build front end

[zackees/soldr](https://github.com/zackees/soldr) wraps cargo and rustup behind
the toolchain pinned in `rust-toolchain.toml`. It bundles **zccache**, a
content-addressed compiler cache shared across branches and worktrees, so the
same crate is not rebuilt for every checkout. It also manages the
cross-compilation SDKs that let a Linux host check and link the Windows and
macOS targets.

What to do:

- `soldr cargo build|test|clippy|fmt …`, always from the repo root.
- `soldr rustup target add <triple>` for an extra target. Don't add targets to
  `rust-toolchain.toml`.
- `soldr status`, `soldr doctor` when something looks wrong; `soldr gc` when it
  warns about disk space. The local gate wants 10 GiB free.

What not to do:

- Bare `cargo` / `rustc`. A hook (`ci/hooks/tool_guard.py`) denies them, and
  `ci/test_tool_guard.py` fails scripts that use them.
- Bump `rust-toolchain.toml` past what the CI soldr supports natively.
  `ci/test_toolchain.py` enforces this; bump soldr first.
- Running cargo inside `rust/`. The workspace is the repo root, and cargo then
  fails to load the workspace (`failed to load manifest for workspace member
  rust/target`).

Known pitfall: zccache has replayed stale compiler output. The local gate runs
cargo with `SOLDR_RUSTC_WRAPPER=none` (`OBS_GATE_ZCCACHE=1` opts back in). If a
build behaves impossibly, retry with that variable set before debugging the
code.

#### Linux C/C++ and PCH caching

The Ubuntu build, AppImage, sanitizers and Tier 2 use the standalone zccache
pinned in `rust/tools/linux-validate/install-zccache.sh`, with SHA-256 checks
for both Linux architectures. Version 1.15.5 restores a byte-identical PCH
input header's recorded mtime before clang loads it; real content changes
remain subject to clang's validation. This fixes `#pragma once` redefinitions
after touches or checkouts without relaxing PCH validation.

This launcher is independent of soldr's embedded Rust cache. The pinned soldr
0.9.35 (and released 0.9.37) embeds zccache 1.15.4. Updating the standalone
installer does not update soldr's `zccache-soldr` wrapper selected by the
optional `ENABLE_ZCCACHE` CMake module. That wrapper needs a soldr release
whose dependency and vendored zccache both include the new version.

#### Cross-compile check: Windows and macOS C from any host

`cmake/cross/` compiles libobs for another OS on this machine, with no Windows or macOS runner.
- **Toolchain:** soldr's LLVM and the SDK soldr provisions for that OS (the Apple SDK, or the xwin MSVC CRT + Windows SDK).
- **Dependencies:** the same prebuilt Windows/macOS dependencies the real build uses (pinned in `CMakePresets.json`, unpacked into `.deps/`).
- **Sources:** the same file lists as the real build (`libobs/cmake/sources.cmake`), including both the C build's ported files and the Rust build's C halves.
- **Output:** objects only; it compiles and links nothing.

```sh
cmake -S cmake/cross -B build_cross_win-x64 -G Ninja -DOBS_CROSS_TARGET=win-x64
cmake --build build_cross_win-x64
# or from a configured OBS build:
cmake --build build --target cross-win-x64      # cross-mac-arm64, cross-mac-x64, cross-all
```

- **Only Docker on the host:** `cmake/cross/docker.sh mac-arm64 win-x64` runs the same check in a container (through `ci/docker-task.sh`).
  - The checkout is a copy-on-write overlay at `/src`.
  - Writable caches are named volumes the next run reattaches: `/build` per checkout; soldr's LLVM and SDKs and the prebuilt deps shared.
  - `--clean` starts the build volume over.
- **Warnings** are reported; `-DOBS_CROSS_WERROR=ON` makes them errors.
- **Getting soldr:** `cmake/common/soldr.cmake` uses soldr from `PATH`, or downloads the version CI pins and checks its SHA256 (`ci/test_soldr_pin.py` keeps that pin equal to the workflows').
- **Finding the toolchain:** it runs `soldr toolchain prepare`, then finds LLVM and the SDK under soldr's root. That changes to `soldr cc --target <triple>` once soldr exposes Apple and MSVC there.
- **No `win-arm64`:** `util/threading-windows.h` uses MSVC-only ARM64 intrinsics that clang lacks.

### setup-soldr: soldr in GitHub Actions

[zackees/setup-soldr](https://github.com/zackees/setup-soldr) is the action
the workflows use to install soldr, the pinned toolchain and its caches. Its
cache layers:

- the toolchain;
- the cargo registry;
- a "cook" layer of prebuilt dependencies, keyed on `Cargo.lock`, the rustc
  release and the soldr version;
- the zccache compiler cache, which lets a branch reuse its parent's artifacts.

Remote caches are saved only when the run ref is `refs/heads/master`,
including master pushes and nightly/manual runs on master. PRs, merge groups
and feature-branch release verification restore but do not save. Every
setup-soldr call sets `save-cache-remote` explicitly: its upstream `auto`
default excludes PRs but still permits queue and feature-branch uploads.
Local act/act2 cache saves stay enabled. A PR's first build depends on
master's cache being warm.

What to do: keep the pinned action SHA and the `version:` input in
`.github/workflows/build-project.yaml` in step with your local soldr and
`rust-toolchain.toml`. Change them together, in one PR.

### bosn: Docker under supervision

[zackees/bosn](https://github.com/zackees/bosn) is a machine-wide daemon that
owns Docker on a shared host. It keeps a registry of containers, volumes and
leases, runs jobs in a bounded pool of runner slots (each capped in CPU), and
tears down what a job leaves behind. It exists so that many agents on one
machine don't each start unbounded Docker work.

What to do:

- `bosn jobs` to see what is running before starting heavy work.
- `bosn ci run --workflow <file> --job <job> --trigger pr --wait`, then
  `bosn ci report <run>`, to run a CI job locally (through act2, below) instead
  of on GitHub.
- `bosn ci doctor` when the local runner looks unhealthy.

What not to do:

- `docker system prune`. It destroys everyone's warm caches.
- Install bosn unasked, or add stacks to `bosn.toml` without a reason. It's
  manifest-driven, and the machine owner decides what runs.

The repo's `bosn.toml` declares one stack so far: `appimage`, an Ubuntu 26.04
image (`build-aux/appimage/Dockerfile`) with the AppImage build dependencies
preinstalled. `bosn run --task appimage` builds the AppImage locally by
running `build-aux/appimage/build.py`, the same script the AppImage workflow
calls, with no act and no GitHub involved. The source is bind-mounted, the zccache
and cargo caches live in bosn volumes, and the AppImage lands in
`build_appimage/` (#899). (`bosn ci run` can't run that job through act: its
`ubuntu-26.04` label isn't supported there.)

Tier 2 Linux in CI runs natively on the runner (#966): the
`rust-linux-tier2` job installs `rust/tools/linux-validate/apt-packages.txt`
and runs `validate.sh` directly: one build, then one job per libobs (C or with Rust) and shard. The
intended local equivalent is bosn running that same job through act2:

```sh
bosn ci run --workflow build-project.yaml --job rust-linux-tier2 --trigger pr --wait
```

It does not run yet: bosn accepts only the `ubuntu-latest`, `-24.04` and
`-22.04` labels (zackees/bosn#642), so it marks the
job's `check-event` and `ci-mode` dependencies unsupported and skips Tier 2
("some jobs need runners bosn cannot supervise"). Until it does, use
`./unittest` (the Docker lane below) locally.

Remaining gap: `rust/tools/linux-validate/run.sh` (the local gate's Tier 2
lane, `./unittest`) still calls `docker build` and `docker run` directly, so
concurrent local gates from many agents run unmanaged. It reads the same
package list; moving it under bosn is the open step.

### act2: GitHub Actions, locally

[zackees/act2](https://github.com/zackees/act2) is a fork of nektos/act that
runs workflow jobs on your own machine, with fixes for soldr's and
setup-soldr's caching. bosn drives it; you don't call it directly. Use it, via
`bosn ci run`, when you need to know whether a real workflow job passes:
don't push a commit just to find out. A cold run is slow (tens of minutes),
so it's an on-demand check, not part of every push. Remote-only jobs (for
example macOS) are reported as such, not faked.

### zackees/ci.yml: the CI policy and its checker

[zackees/ci.yml](https://github.com/zackees/ci.yml) is the set of rules this
repo's CI follows, plus `ci-lint`, its checker and local-gate runner. The core
ideas:

- **Two cycles.** The dev cycle (every PR and push) runs a quick, mostly-Linux
  gate. The release gate (nightly and on release) runs everything: every
  platform, full app builds, Flatpak, packaging. Expensive jobs live in the
  release gate.
- **Local first.** The quick remote gate must be something you can run locally
  with one command, so a green local run predicts a green PR.
- **Attest instead of repeat.** A local run stamps the commit; CI trusts the
  stamp and skips what it already proved (below).

Always use the pinned version from [AGENTS.md](../AGENTS.md):

```
uvx --from git+https://github.com/zackees/ci.yml@1a970f01574447d681571ff3d0b8de56cb983dcc ci-lint local-gate run
```

### ci.toml and local-gate.toml: what runs for which change

In ci.yml, `ci.toml` (schema 3) declares a repo's platforms, test suites and
the paths that select them, so a change only triggers the tests it can affect.
**This repo has no `ci.toml`.** It uses two pieces that do the same job:

- **`local-gate.toml`** declares the local lanes and what they attest:
  - `lint`: formatters, clippy, dylints, the `ci/` Python tests;
  - `rust-test`: Rust Tiers 1-3;
  - `cross`: clippy, dylint, and a link of every test binary
    (`test --no-run`) for the Windows and macOS targets, cross-compiled from
    Linux with soldr;
  - `tier2-linux`: the cmocka suite against libobs and libobs-with-rust, in
    Docker.
- **`ci/ci_scope.py`** maps changed paths to jobs, by path alone (#897). Linux
  Rust tests and Tier 2 always run. A changed file that belongs to a platform
  (`ci/platform_paths.py`: path tokens such as `win-capture/`,
  `platform-osx.mm`, `rust/*/src/platform/windows/`, or extensions such as
  `.m`, `.rc`) starts that platform's Rust tests and Tier 2. POSIX/Unix files
  (`*-posix.c`, `unix/`, and `libobs/util/platform-nix.c`) belong to Linux
  and macOS, so changing one starts macOS too. Core files carry no platform
  defines or OS headers (`ci/platform_ifdef_ratchet.py` enforces it, and
  fails a platform file that names another OS), so a core change runs on
  Linux; file contents are never read. A module whose name doesn't say its OSes
  (`coreaudio-encoder`, `obs-text`, ...) is listed in `ci/platform_paths.py`
  with its `PLATFORMS` from `plugins/CMakeLists.txt`; upstream paths are
  never renamed for this (#1051). `libobs/cmake/os-<os>.cmake` starts only
  its OS. Shared and so every platform:
  `.github/`, `cmake/common/`, `cmake/finders/`, the rest of `libobs/cmake/`, the root
  CMake files, `platform/mod.rs`, `test/cmocka/CMakeLists.txt` and the C++
  cmocka tests with their `tests.d` registrations (#610). Pushes and the
  merge queue are scoped the same way; every platform runs on master pushes, schedule, workflow_dispatch, tags,
  release branches and in the release gate; no PR path selects the full builds.
  Packaging and submodule moves are still compiled on Linux (touched targets)
  on the PR. `.github/` edits start no platform job.

### Path to lane table (fast gate)

A PR needs about 5 runners: Event Data, CI Mode (attestation and lane proofs),
Checks (swift detection, flatpak and Qt XML validators, ci/ self-tests; plus
clang-format and gersemi on pushes), the Fast Gate and CI Gate. The Fast Gate
runs formatters, rustfmt, clippy, Dylint and the unit tests of the crates the
diff touches and their dependents (`ci/rust_scope.py`; libobs, dylints and
Cargo files test the workspace), on every PR. No Qt is installed there.
`LANES` in `ci/ci_scope.py` selects the rest by path:

| Paths | Lane |
|---|---|
| everything | Fast Gate: 1) fmt, clippy, Dylint; 2) unit tests of the touched crates; 3) Linux build of touched `plugins/<x>`/`shared/<x>` targets (ENABLE_FRONTEND=OFF, no Qt) |
| `libobs/` (not platform-only files), crates linked into libobs, `test/cmocka/`, `deps/`, `rust/tools/linux-validate/`, Cargo files | Tier 2 Linux: compilation overlaps Fast Gate; OFF/ON tests wait for it |
| `frontend/`, Qt-linked tests, `shared/qt`, the Qt job's own files | Qt Frontend Linux (the only Qt). Frontend platform files start no Windows/macOS Rust or Tier 2 job; Windows/macOS Qt lanes are a follow-up |
| files matching `/platform[/-](windows, macos, linux, posix)[/.]` (posix = linux+macos), legacy OS names, shared build files (`cmake/common`, `cmake/finders`, `libobs/cmake`, root CMake, `platform/mod.rs`, C++ cmocka tests) | macOS / Windows Rust tests, Tier 2, cross lint |
| `build-aux/`, `.gitmodules`, submodules | compiled on Linux only; no full build from a PR |
| `docs/`, `.github/`, `ci/` | lint and the ci/ self-tests |

The macOS, Windows, Qt and cross jobs wait for Fast Gate. Tier 2 Linux
compilation is the explicit exception: it can overlap on an independent runner
after scope/proof checks. OFF/ON execution waits for both the build and a
successful Fast Gate, or its valid attestation skip. Failed/canceled Fast Gates
start neither test suite. The final required gate still includes every check.
This trades possible wasted compilation on a red PR for a shorter successful
queue path; a compile already running can finish after Fast Gate fails.
Measure runner cost and actual gate time rather than treating the theoretical
overlap as a proven speedup (#930, #1286).

Unchanged lanes are cached by the gate, so a re-run after a small fix is fast.

### Lane proofs: skipping inputs that already passed

A local-gate stamp covers one exact tree, so it can never cover a merge-queue
commit. Lane proofs (`ci/lane_proof.py`, #699) are keyed by a lane's
*inputs* instead:

- Each lane declares what it reads in `local-gate.toml` (`exclude`/`keep`,
  checked by tracing the lane with strace). `rust-test` and `cross` read
  `rust/`, `dylints/`, `libobs/`, `deps/`, `plugins/obs-nvenc/` and the Cargo
  and toolchain files; `tier2-linux` reads everything but docs, packaging,
  the non-Linux graphics backends and CI-only files. `lint` reads everything.
- When `Rust Tests (Linux)`, `Rust Cross Lint` or `Rust Tier 2 Linux` pass, the
  job uploads an empty artifact named after the lane and its input key.
- On a PR or merge-queue run, `Lane Proofs 🧾` recomputes each key, with the
  lane declarations read from the base commit, and skips a lane whose key
  has a proof from a trusted run: this repository's PR workflow on a branch
  in this repository, or a merge-queue run, within 72 hours. Fork runs
  never publish trusted proofs, but fork PRs do reuse proofs.
- No proof is used when the change touches a proof surface (`local-gate.toml`,
  `ci/lane_proof.py`, `ci/local_gate.py`, workflows and actions, the Tier 2
  harness, `rust-toolchain.toml`), on pushes, schedules, dispatches and full
  runs, or when the lookup fails.

Over the last 200 master commits, the Rust lanes' inputs were unchanged 63% of
the time and Tier 2's 18%, so a queue rebuild after an unrelated merge
usually reruns only lint and the event checks. Check a key with
`uv run ci/lane_proof.py key --lane rust-test --base origin/master`.

### CI attestation: skipping what's already proven

When `ci-lint local-gate run` passes, it adds `Local-Gate` trailers to HEAD
that record the exact tree and the lanes that ran. In PR CI the `CI Mode` job
verifies the stamp, and if it holds, CI skips the jobs those lanes cover:

| Job skipped | Lanes that cover it |
|---|---|
| Rust Tests (Linux) | `lint` and `rust-test` |
| Rust Tier 2 Linux | `tier2-linux` |
| Rust Cross Lint | `cross` |

**Never skipped by a stamp:** the event checks, the macOS, Ubuntu, Windows and
Flatpak app builds, and the platform Rust test and Tier 2 jobs that
`ci_scope.py` selects.

**The stamp is ignored (CI runs everything it would anyway) when:**
- the PR comes from a fork or from a non-writer;
- the head isn't attested for its exact tree;
- a lane is missing from the stamp;
- a "surface" changed: the gate itself, the Tier 2 harness,
  `rust-toolchain.toml`, workflows, or the `ci/` scripts the lanes use;
- the PR falls in the 1-in-10 audit sample.

**The stamp is lost by any change after stamping:**
- a content amend;
- a rebase;
- GitHub's "Update branch" button, which creates an unstamped merge commit.

A message-only amend keeps it. After any of these, re-run the gate, or accept
that CI will run the Linux jobs itself.

## Before you push

For a focused CI/configuration bug fix, keep the RED and GREEN commits and
record the local failing and passing commands, but push them together when
the fix is ready. A separate remote RED run is not required. This avoids
spending runner time on an already-fixed failure; see
[coordination.md](rust-port/coordination.md#4-pull-requests).

1. Claim the issue and check open PRs ([coordination.md](rust-port/coordination.md)).
2. Commit RED, then GREEN.
3. `./lint --fix`, then commit what it changed.
4. Check `git diff --cached` for an unintended `plugins/obs-websocket` pointer
   change. A pointer that doesn't descend from master's is rejected, and stale
   pointers have reverted merged fixes before.
5. Run the local gate from a clean, committed tree. If it reports another gate
   running for this clone, wait and retry.
6. Push the stamped commit as-is: no amend, no rebase, no "Update branch"
   afterwards. Rebase before stamping, not after.
7. There is no label to force CI: a `build-aux/` or submodule change selects
   the full builds by path.

## When CI fails

- Read the failing job's log first: `gh run view <run> --job <job> --log-failed`,
  or the job log via the API while the run is still going.
- Reproduce locally (`./unittest`, the relevant gate lane, or `bosn ci run` for
  a workflow job), fix, re-stamp, push once.
- Re-run a single job only for a clear infrastructure flake (a network
  timeout, a runner image download): `gh run rerun --job <id>`. Never re-run a
  whole workflow to "see if it passes this time".
- Platform-only failures (macOS, Windows) can't be reproduced on Linux. The
  Tier 2 harnesses print crash stacks (`cdb` on Windows, `lldb` on macOS) to
  read in the log; use those before pushing guesses.

## Pre-work, cache maintenance and cancellation

`ci-pre.yml` is the general entry point for pre-work. Pull calls it for every
PR and merge-group run; Push calls it on master. Put each operation's event
condition on that operation's job. Do not skip the whole entry point merely
because one maintenance operation does not apply.

The cache janitor runs only for `merge_group` or a `push` to
`refs/heads/master`. Fork PR tokens cannot delete repository caches, so PR
pushes must skip the janitor job. Keep cache deletion out of untrusted PR
events; do not switch to `pull_request_target` to obtain a write token.

Cache cleanup is best-effort maintenance. It has no dependents in the Pull
or Push workflow and is excluded from `CI Gate ✅`. Its own failures remain
visible for investigation, but cannot delay builds, queue cancellation or
merging. Keep the repo-wide `cache-janitor` concurrency group with
`cancel-in-progress: false`. A future pre-work job that is required for
correctness needs its own explicit dependency design; the janitor's policy
does not make all future pre-work optional.

Merge-queue throughput takes priority over housekeeping. Superseded-run
cancellation must start without waiting for cleanup. Let workflow
concurrency eagerly cancel obsolete PR runs; if a superseded run still
occupies the slot, request its cancellation rather than waiting for a known
obsolete result. GitHub cancellation is asynchronous, so confirm the old
run has stopped before calling it canceled. Cancel obsolete work, not a
current required check simply because it is red. A janitor failure is a
maintenance incident; a failure of the current required gate blocks normal
queueing and needs investigation.

Changes to this policy need corresponding updates in `ci/test_ci_pre.py`
and the workflow call sites. Decisions and evidence for the current behavior
are tracked in [#1269](https://github.com/obs-rust/obs-studio/issues/1269)
and [#1270](https://github.com/obs-rust/obs-studio/pull/1270).

## Where this is going (open gaps)

For a reproducible traffic/cache snapshot, run the read-only collector:

```sh
uv run ci/cache_metrics.py --output build_ci_audit/metrics \
  --start 2026-10-16T17:00:00Z --end 2026-10-17T17:00:00Z
```

It retains raw API responses, actual changed-file cohorts, sample IDs,
listing coverage, per-lane job/setup medians, gate wall time and canceled
runner-minutes. Compare code PRs and merge groups separately from docs and
CI changes. Unknown source diffs remain unknown; sparse samples and changes
to source/configuration between windows need to be reported. Its cache
inventory describes collection time, not historical inventory at the end
of a past window. See the [2026-10-17 audit](ci-cache-audit-2026-10-17.md).

Each gap is a step toward the full protocol, and a good issue to pick up:

- **Docker through bosn.** The Tier 2 Linux harness
  (`rust/tools/linux-validate/run.sh`) drives Docker directly, so concurrent
  local gates run unmanaged. Running it under bosn's bounded runner slots is
  the biggest single relief for shared machines. (The AppImage build already
  runs under bosn: `bosn run --task appimage`.)
- **A real `ci.toml`.** Path selection lives in `ci/ci_scope.py`, outside
  ci.yml's checker. Moving it into a schema-3 `ci.toml` lets the protocol's
  tooling validate and evolve it.
- **Attested platforms.** The local gate covers Linux only, so macOS and
  Windows changes always cost PR CI time. Native or replayed lanes that can
  attest those jobs would let CI skip them too (see issue #443).
- **Warm PR caches.** setup-soldr restores but doesn't save on PRs, so a
  branch's first build depends on master's cache being warm.
- **Feed lessons back.** Problems found here, such as the stale zccache
  replay or the gate lock contention, belong in the tool's own repo, so every
  project using the protocol benefits.

## Cook profiles and post-job snapshots

All setup-soldr consumers declare their cook flags. Static-library and
release jobs cook release dependencies. Native test and cross-check jobs
use the dev profile; cross checks still install their target standard
libraries through the existing gate, and host cooking does not imply
that a cross-target dependency tree is prebuilt.

Fast Gate retains release cooking for core changes and uses dev cooking
when workspace core inputs are unchanged. The local post-job benchmark
(#1317) measured unchanged medians of 57.259 s release, 36.170 s dev and
72.136 s without cook; core rebuild medians favored release but varied
widely. This is a policy to validate in live CI, not a universal profile
speed claim. The action hashes profile flags into the cook cache key, so
the dev rollout gets a separate archive generation. Master must seed it
before later PRs can get a warm restore.

The action saves the cook snapshot after consumers run. Thus a release
archive can also carry dev artifacts. Keep dependency cooking enabled
unless setup plus consumer measurements demonstrate that removing it is
faster; do not decide from the profile name alone.

## Background master build concurrency

Merge-queue CI remains the required merge check. Full post-merge master
builds warm caches and validate deferred lanes, but overlapping generations
compete with the queue for runners. The Push workflow therefore applies
concurrency only to its `build-project` reusable-workflow call: one active
master build finishes, and GitHub retains only the newest pending build.
Tag/release-branch calls use unique run IDs and remain independent.

Keep `cancel-in-progress: false`: continuous merges must not repeatedly
interrupt active cache saves and prevent warming. Intermediate pending
master build calls can be cancelled as superseded; all required PR and
merge-group checks still run under their own policy. This trades per-commit
background coverage for latest-snapshot validation and less runner
contention. A plugin change in a superseded pending generation may wait
for later work to warm that scoped plugin cache. Measure runner wait and
queue wall time before claiming a throughput gain.

Do not move this group to Push workflow scope. `ci-pre` remains an
independent general pre-work entry point, and its janitor keeps its own
non-cancelling repository-wide concurrency policy. A completed run's
`cancelled` pending janitor job is not evidence that its deletion script
failed: inspect whether it ever started before attributing the outcome.

GitHub documents [job concurrency on reusable-workflow calls](https://docs.github.com/en/actions/reference/workflows-and-actions/reusing-workflow-configurations)
and [the default single-pending concurrency behavior](https://docs.github.com/en/actions/concepts/workflows-and-actions/concurrency).

### Compiler and cache log evidence

Add `--include-logs` to `ci/cache_metrics.py` to collect sampled terminal
runs' latest-attempt logs alongside the existing API responses. Raw logs
are cached per run/attempt, and each record/summary includes recognized
per-job archive byte observations, compiler reports, saved keys and CTest
counts. Missing logs or counters remain unknown.

A restored archive does not imply compiler hits: one observed zccache
rollout restored 278 MB but reported 772 cold compilations. Keep C/C++
build totals separate from soldr session counters and ccache reports;
their scopes differ. Archive messages can repeat and must not be summed
as network transfer totals. Stratify compiler-version rollouts and cold
cache seeds before comparing warm lane performance.

Master build coalescing retains changed paths since the latest successful
ancestor Push run, rather than only the latest push's parent. A replaced pending
master build therefore cannot drop plugin/platform cache warming. If no recent
successful ancestor can be established, scope all tracked paths conservatively;
API failure must not silently omit work. This changes master warming only;
PR and merge-group scope still use their own bases.

## Merge queue admission policy

The merge queue prioritizes compilation errors and integrity regressions:
changed-path compilation, formatting/lint correctness, unit tests, ABI and
behavior checks, and the selected C/Rust differential and Tier 2 suites.
`CI Gate ✅` is the required merge check. Keep its dependencies limited to
these checks. Cache maintenance stays independent and cannot block this gate.

Security analysis (CodeQL, vulnerability/dependency audits and static security
scanners), release packaging validation, signing, publishing and distribution
checks belong in the existing nightly workflows, with manual dispatch available
for diagnosis. They must not run on PR pushes or `merge_group`, even as optional
background jobs: they compete for runner capacity. Explicit release/tag actions
may still validate and publish releases outside the merge queue.

GitHub CodeQL **default setup must remain disabled** in repository settings;
it creates automatic PR/push scans outside the versioned workflow graph.
The existing `scheduled.yaml` owns nightly CodeQL for Actions, C/C++, Python
and Rust. Any new security integration must be reviewed for its event triggers,
required-check settings and runner use before enabling it. Do not add a security
scan to `CI Gate`, merge rules or queue cancellation dependencies. Nightly
failures are investigated through nightly reports rather than blocking the queue.
