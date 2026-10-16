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

### setup-soldr: soldr in GitHub Actions

[zackees/setup-soldr](https://github.com/zackees/setup-soldr) is the action
the workflows use to install soldr, the pinned toolchain and its caches. Its
cache layers:

- the toolchain;
- the cargo registry;
- a "cook" layer of prebuilt dependencies, keyed on `Cargo.lock`, the rustc
  release and the soldr version;
- the zccache compiler cache, which lets a branch reuse its parent's artifacts.

On pull requests the caches are restored but not saved; default-branch pushes
save them. So a PR's first build is only fast if master's cache is warm.

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
calls, with no act and no GitHub involved. The source is bind-mounted, ccache
and cargo caches live in bosn volumes, and the AppImage lands in
`build_appimage/` (#899). (`bosn ci run` can't run that job through act: its
`ubuntu-26.04` label isn't supported there.)

Tier 2 Linux in CI runs natively on the runner (#966): the
`rust-linux-tier2` job installs `rust/tools/linux-validate/apt-packages.txt`
and runs `validate.sh` directly, one job per `ENABLE_RUST_LIBOBS` mode. The
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
  - `tier2-linux`: the cmocka suite with `ENABLE_RUST_LIBOBS` OFF and ON, in
    Docker.
- **`ci/ci_scope.py`** maps changed paths to jobs, by path alone (#897). Linux
  Rust tests and Tier 2 always run. A changed file that belongs to a platform
  (`ci/platform_paths.py`: path tokens such as `win-capture/`,
  `platform-osx.mm`, `rust/*/src/platform/windows/`, or extensions such as
  `.m`, `.rc`) starts that platform's Rust tests and Tier 2. POSIX/Unix files
  (`*-posix.c`, `unix/`, and `libobs/util/platform-nix.c`) belong to Linux
  and macOS, so changing one starts macOS too. Core files carry no platform
  defines (`ci/platform_ifdef_ratchet.py` enforces it), so a core change runs
  on Linux; file contents are never read. Shared and so every platform:
  `.github/`, `cmake/common/`, `cmake/finders/`, `libobs/cmake/`, the root
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
| `libobs/` (not platform-only files), crates linked into libobs, `test/cmocka/`, `deps/`, `rust/tools/linux-validate/`, Cargo files | Tier 2 Linux (after the Fast Gate) |
| `frontend/`, Qt-linked tests, `shared/qt`, the Qt job's own files | Qt Frontend Linux (the only Qt). Frontend platform files start no Windows/macOS Rust or Tier 2 job; Windows/macOS Qt lanes are a follow-up |
| files matching `/platform[/-](windows, macos, linux, posix)[/.]` (posix = linux+macos), legacy OS names, shared build files (`cmake/common`, `cmake/finders`, `libobs/cmake`, root CMake, `platform/mod.rs`, C++ cmocka tests) | macOS / Windows Rust tests, Tier 2, cross lint |
| `build-aux/`, `.gitmodules`, submodules | compiled on Linux only; no full build from a PR |
| `docs/`, `.github/`, `ci/` | lint and the ci/ self-tests |

The macOS, Windows, Qt, Tier 2 and cross jobs `needs:` the Fast Gate, so a lint or unit-test failure spends none of those runners.

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

## Where this is going (open gaps)

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
