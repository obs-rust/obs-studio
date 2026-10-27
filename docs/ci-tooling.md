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
2. **The local gate is optional.** If you run it, push the stamped commit
   unchanged: a valid stamp lets PR CI skip the Linux test jobs. Without a
   stamp, PR CI runs them itself.
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

Known pitfall: zccache has replayed stale Rust compiler output. Both the build
and release workflows set `SOLDR_RUSTC_WRAPPER=none` for setup/cook and native
Rust consumers; the local gate sets it too (`OBS_GATE_ZCCACHE=1` opts back in
locally). Pinned soldr 0.9.43 defaults to the managed wrapper without this
override, and [zccache#1943](https://github.com/zackees/zccache/issues/1943)
remains unresolved. This safeguard preserves standalone C/C++ compiler
caching and soldr cook/target archives. All seven setup-soldr callsites also
set `build-cache: false`: the full-run audit observed no embedded session
across all eight expanded build lanes, despite transferring compiler-store
archives (42.9 MB and 1.1–2.2 s in the inspected Linux host jobs). This input
removes embedded compiler-store transport; it preserves cook, registry and
toolchain layers. The unused `setup-soldr:build-cache` retention family is
also retired, allowing the janitor to reclaim legacy archives rather than
protecting them as a family's newest entry. The October 18 inventory held
three such archives totaling 183,330,575 bytes. This is reclaimable storage,
not proof of repository-wide headroom or a measured CI speedup. Reconsider
the layer and declare its family again only when the compiler correctness probe
passes and a consumer demonstrates useful reuse. The release callsite follows
the same bypass policy; no release runtime comparison is claimed.
Soldr 0.9.38 fixes source mtime truncation during archive replay (#1359).
Regenerated local `obs-util` full and base plus delta archives preserve the
exact nanosecond timestamp and report Fresh with the wrapper disabled. The
pinned setup action's cook keys include the Soldr version, so archives of an
older Soldr cannot match the new base key or delta restore prefixes. The pinned
setup-soldr v0.9.88 defaults to Soldr 0.9.42; the workflows still name the
version explicitly. See the [probe and limits](ci-evidence/2026-10-20-soldr-source-mtime.md).

Soldr 0.9.41+ keeps a **verdict ledger** (soldr#3894): a `clippy`,
`fmt --check`, `dylint` or doctest run whose exact inputs (git blob ids, or
content hashes for uncommitted files, plus toolchain, argv and environment)
already passed is replayed instead of re-run, narrowed per workspace member.
setup-soldr carries it between CI runs (`verdicts: auto`, the default): master
publishes, PRs restore. Every setup-soldr step in `build-project.yaml` sets
`verdict-scope: build-project`: the manifest scope otherwise hashes the
calling workflow (`push.yaml` on master, `ci.yml` on PRs), so PRs never found
master's verdicts (forge-ci/setup-soldr#604, `ci/test_verdict_scope.py`).
In CI the first master run with it (40159829966) published 8 verdicts under
scope `b88ba65d043f3f19`, the scope PR and merge-queue Fast Gate runs read.
It is content-keyed, so it does not depend on the
wrapper bypass above. `SOLDR_VERDICTS=off` forces a run. The gate's source
probe sets it, because before forge-ci/soldr#3913 a scratch crate in an ignored
directory (`target/`) keyed on none of its files and replayed a stale pass.
The lint lane runs the Dylints through `soldr dylint` with soldr's managed
driver (`SOLDR_FORCE_MANAGED_CARGO_SUBCOMMANDS=1`) in `target-dylint/`, beside
the stable Clippy steps (#2293).
This does not solve the separate
restored-target fingerprint hazard documented in the cache audit. If a
build behaves impossibly, retry with that variable set before debugging the
code.

#### Linux C/C++ and PCH caching

Ubuntu app compiler snapshots restore only within the same runner, architecture,
Rust mode and build configuration. An observed opposite-mode fallback yielded
only 43 cached compilations out of 361 calls and accumulated 9,526 MiB, exceeding
the 8,192 MiB save guard and preventing an OFF seed. A missing compatible seed
starts cold; publication remains subject to the post-build size guard and
retention policy. This restriction does not establish that both modes fit the
current pooled nightly retention allowance.

The Ubuntu build, AppImage, sanitizers and Tier 2 use the standalone zccache
pinned in `rust/tools/linux-validate/install_zccache.py`, with SHA-256 checks
for both Linux architectures. Version 1.15.5 restores a byte-identical PCH
input header's recorded mtime before clang loads it; real content changes
remain subject to clang's validation. This fixes `#pragma once` redefinitions
after touches or checkouts without relaxing PCH validation.

The pinned soldr 0.9.43 also embeds zccache 1.15.5, including the wrapper
selected by the optional `ENABLE_ZCCACHE` CMake module. Updating the standalone
installer alone does not update that embedded wrapper. Matching the versions
does not establish Rust compiler-cache correctness: the Rust wrapper bypass
above remains enabled.

#### Qt compiler snapshot publication

Qt compiler snapshots use `qt-ccache-linux-inputs-v2-<digest>` rather than a
new key for every commit. On the clean CI checkout, the digest includes staged
file identities except `docs/`, Markdown, `.github/`, `ci/`, and `ci.toml`;
the build workflow and `ci/linux_qt_frontend.py` are explicitly included again.
Thus CI policy/test metadata does not cause an upload, while workflow settings,
dependency installation and the Qt build commands remain publication inputs.
The digest also includes the runner image, pinned Qt/FFmpeg
archives, and installed C, C++, CMake, ccache and Ninja versions. An exact restore
skips the upload; changed inputs can restore the existing `qt-ccache-linux-`
fallback and publish a new snapshot on master. Older archives are not purged.

This is a publication identity, not a build or test attestation. Compilation
and Qt tests still run, and ccache validates current compiler inputs. Generated
commit-version objects can still miss after docs-only commits; avoiding another
~1.16 GB archive for those objects is intentional. It does not make deeper Rust
test archives safe or solve repository-wide retention by itself. Runtime
publication/reuse measurements remain required after deployment.

After the Qt build/tests, the runner's compiler cache evicts entries unused
since preparation began, with a one-minute margin, then applies its existing
650M size limit. The measured pruned warm working store is 508,489,050 bytes;
650M leaves over 25% working headroom, with a 750MB retention allowance in
`ci.toml` (ci-lint interprets MB as MiB). Hits refresh ccache entry timestamps, so current-build hits
and misses remain eligible for the next snapshot while stale objects can be
removed. Missing preparation timestamps skip age eviction. Before/after byte
counts are logged on PRs as well as master; remote uploads remain master-only.
These filesystem counts are not compressed archive sizes or proof of the next
consumer's hit rate. Required builds/tests always run and failed jobs do not
publish a cache.

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

- **Only Docker on the host:** `uv run --script cmake/cross/docker.py mac-arm64 win-x64` runs the same check in a container (through the `ci/docker-task.sh` shim into `ci/docker_task.py`).
  - The checkout is a copy-on-write overlay at `/src`.
  - Writable caches are named volumes the next run reattaches: `/build` per checkout; soldr's LLVM and SDKs and the prebuilt deps shared.
  - `--clean` starts the build volume over.
- **Warnings** are reported; `-DOBS_CROSS_WERROR=ON` makes them errors.
- **Getting soldr:** `cmake/common/soldr.cmake` uses soldr from `PATH`, or downloads the version CI pins and checks its SHA256 (`ci/test_soldr_pin.py` keeps that pin equal to the workflows').
- **Finding the toolchain:** it runs `soldr toolchain prepare`, then finds LLVM and the SDK under soldr's root. That changes to `soldr cc --target <triple>` once soldr exposes Apple and MSVC there.
- **In PR CI:** the `Cross Compile 🧰` job in `build-project.yaml` runs it, compile-only, for the aliases `ci/ci_scope.py` selects (`cross_targets`).
  It runs when a change touches a Windows or macOS file in libobs or a component the cross `CMakeLists.txt` names by `DIR`, or the check itself (`cmake/cross/`, `cmake/common/soldr.cmake`); full runs build both (#1294).
  The frontend's Windows and macOS Qt groups are in it (#2408): the target's prebuilt Qt6 headers, with `ui_*.h` from the pinned Linux `uic`; moc is skipped, since nothing links.
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

The repo's `bosn.toml` declares the AppImage stack `appimage-228`, the
glibc 2.28 image (`build-aux/appimage/manylinux/Dockerfile`) the workflow
builds on.
`bosn run --task appimage` builds the AppImage locally on the 2.28 image by
running `build-aux/appimage/build.py`, the same script the AppImage workflow
calls, with no act and no GitHub involved. The source is bind-mounted, the zccache
and cargo caches live in bosn volumes, and the AppImage lands in
`build_appimage/` (#899). (`bosn ci run` can't run that job through act: its
`ubuntu-26.04` label isn't supported there.)

Tier 2 Linux in CI builds once in `rust-linux-tier2-build`, using the pinned
stock Ubuntu job container and full `apt-packages.txt` development inputs.
It transfers both implementations' compiled tree and an exact runtime-package
manifest. `rust-linux-tier2` runs one complete suite per OFF/ON matrix mode,
with no shards. Its host invokes `rust/tools/linux-validate/ci-runtime.py`:
the helper downloads and verifies the immutable prepared runtime pinned in
`prepared-runtime.json`, then runs `validate.sh` in a four-CPU container with
the original source/build paths and writable CTest timing history. Source is
read-only. Each suite retains eight CTest workers and existing serial isolation.

A changed package closure or unavailable release asset selects installation
in the same pinned stock base. Archive size/checksum, image identity or inventory
errors fail the job. The existing apt cache remains available for fallback;
test jobs never publish it. Runtime assets use the existing public
`obs-rust/obs-studio-libs` release/catalogue route without additional CI
credentials. Producer and cold-transfer measurements are recorded in
[the speed report](ci-speed-2026-10-22.md#fresh-prepared-runtime-experiment-and-production-candidate-1717).
The intended local equivalent is bosn running the job through act2:

```sh
bosn ci run --workflow build-project.yaml --job rust-linux-tier2 --trigger pr --wait
```

It does not run yet: bosn accepts only the `ubuntu-latest`, `-24.04` and
`-22.04` labels (zackees/bosn#642), so it marks the
job's `check-event` and `ci-mode` dependencies unsupported and skips Tier 2
("some jobs need runners bosn cannot supervise"). Until it does, use
`./unittest` (the Docker lane below) locally.

The local gate's Tier 2 lane (`./unittest`) uses
`rust/tools/linux-validate/run.py` and `ci/docker_task.py` (reached through the
`ci/docker-task.sh` shim), the current stand-in
for zackees/bosn#666. It has per-checkout build volumes and container ownership,
shared compiler/crate caches, and explicit storage budgets. Run local Docker
work through bosn as described below; the prepared CI consumer does not change
the local build harness or its development-package inputs.

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
uvx --from git+https://github.com/zackees/ci.yml@a1917ce76598904ce515cd7eec83450c317ea6f2 ci-lint local-gate run
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
  - `tier2-linux`: the cmocka suite against libobs and libobs-c-reference, in
    Docker.
- **`ci/ci_scope.py`** maps changed paths to jobs, by path alone (#897). The
  Fast Gate always runs; Linux Tier 2 runs for the paths in its `LANES` row
  (table below). A changed file that belongs to a platform
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
  CMake files, crate-root `rust/<crate>/build.rs`, `platform/mod.rs`,
  `test/cmocka/CMakeLists.txt` and the C++
  cmocka tests with their `tests.d` registrations (#610). Pushes and the
  merge queue are scoped the same way. A master push adds the lanes a PR
  defers (Linux Tier 2, Qt, the workspace tests) and runs macOS/Windows
  Rust tests and Tier 2 only for paths that select them; it builds no OBS
  app. Every platform and every app build runs on schedule,
  workflow_dispatch, tags, release branches and in the release gate; no PR
  path selects the full builds.
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
| `libobs/` (not platform-only files), crates linked into libobs, `test/cmocka/`, `deps/`, `rust/tools/linux-validate/`, Cargo files | Tier 2 Linux: compilation overlaps Fast Gate; OFF/ON tests wait for the Tier 2 build, with one complete suite per mode |
| `frontend/`, Qt-linked tests, `shared/qt`, the Qt job's own files | Qt Frontend Linux (the only Qt). Frontend platform files start no Windows/macOS Rust or Tier 2 job; Windows/macOS Qt lanes are a follow-up |
| files matching `/platform[/-](windows, macos, linux, posix)[/.]` (posix = linux+macos), legacy OS names, shared build files (`cmake/common`, `cmake/finders`, `libobs/cmake`, root CMake, crate-root `build.rs`, `platform/mod.rs`, C++ cmocka tests) | macOS / Windows Rust tests, Tier 2, cross lint |
| `build-aux/`, `.gitmodules`, submodules | compiled on Linux only; no full build from a PR |
| `docs/`, `.github/`, `ci/` | lint and the ci/ self-tests |

The macOS, Windows and cross jobs wait for Fast Gate. Qt starts on an
independent runner after successful scope detection; its compilation and tests
can overlap Fast Gate (#1581). Tier 2 Linux compilation also overlaps on an
independent runner after scope/proof checks. OFF/ON execution requires a
successful Tier 2 build and a non-canceled workflow; it can overlap remaining
Fast Gate work. A failed or skipped build starts neither suite. Fast Gate
remains independently required by the final gate, so its failure prevents
merging even if both suites pass. This trades possible wasted compilation,
Tier 2 and Qt tests on a red PR for a shorter successful queue path;
independent work can finish after Fast Gate fails.
Measure runner cost and actual gate time rather than treating the theoretical
overlap as a proven speedup (#930, #1286).

Tier 2 uses one complete suite per mode (#1662), each in its own runner
environment: two test jobs rather than four. Deinterlacing retains
`RUN_SERIAL` because CI still dropped fields with bounded software-renderer
workers. The other nine former barriers
are removed: eight tests already own their resources, and XSHM now uses a
private Xvfb (serial fallback when unavailable). Linux tests pin
`LP_NUM_THREADS=1`; see the [concurrency audit](ci-serial-audit-2026-10-21.md).
The build job produces one tree for both backends. Test jobs run the entire
CTest registry without a shard selector, with up to eight binaries concurrent
within each suite. A failed or canceled backend cannot publish a passing lane
proof. Timings have unique per-mode artifacts and merge on master; recorded
costs still order the longest tests first. Optional local sharding tools remain
available, including their complete-partition checks.

The change removes two repeated runner setups and artifact downloads. A full
suite can take longer than an individual shard, so fewer jobs alone do not
prove a shorter CI gate. Compare actual gate time, queue delay, coverage and
runner cost in the broader #1581 study before claiming a workflow speedup.

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
  Tier 2's publishing job requests a runner only when a lane proof can be
  published or a master push must merge/save test timings (#1581). If neither
  operation applies, it skips at job level; the OFF/ON test results still
  determine the reusable workflow's result and required gate.
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
5. Optional: run the local gate from a clean, committed tree. If it reports
   another gate running for this clone, wait and retry.
6. If you ran it, push the stamped commit as-is: no amend, no rebase, no
   "Update branch" afterwards. Rebase before stamping, not after. If you
   didn't, just push; PR CI runs the Linux jobs itself.
7. There is no label to force CI. A `build-aux/` or submodule change is
   compiled on Linux only; the full builds run on schedule,
   workflow_dispatch, tags and the release gate.

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
`refs/heads/master`. `ci-pre.yml`'s janitor sweeps master pushes. The merge
queue sweeps at the end of the run, in `ci.yml`'s final `cache-janitor` job
(`needs` every other job, `if: always()`), never at the start (#1854). The
queue serializes master's cache writes, so that sweep has the cache to itself
and sees what the run saved. Pinned to zackees/ci.yml#405, it evicts the
least recently accessed entries until the repository total fits
`[cache].budget`, keeping each family's newest entry and anything younger
than 10 minutes, so GitHub's own LRU eviction never chooses what to drop.
Declared family maxima may add up to more than the budget; the end-of-queue
sweep is what keeps live usage under it. Fork PR tokens cannot delete
repository caches, so PR pushes must skip both janitor jobs. Keep cache deletion out of untrusted PR
events; do not switch to `pull_request_target` to obtain a write token.

Cache cleanup is best-effort maintenance. It has no dependents in the Pull
or Push workflow and is excluded from `CI Gate ✅` (the end-of-queue sweep
runs after the gate, which never waits on it). Its own failures remain
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

`setup-uv` follows the same remote writer policy: each call sets
`save-cache: ${{ github.ref == 'refs/heads/master' }}` while keeping restores
enabled wherever caching was already enabled. The pinned action otherwise
saves on PR and queue refs, producing archives the janitor later reclaims.
The [retention snapshot and offline executor reproduction](ci-evidence/2026-10-21-cache-retention.md)
record that churn separately from the still-open whole-budget allocation and
durability work. A current inventory below the global budget does not prove
that useful seeds survive the next nightly writer burst.

The [24-hour PR and merge-group cohort](ci-evidence/2026-10-22-ci-cache-day-cohort.md)
records gate latency, failures, cancellations and incomplete timing proxies
separately for each event. Its companion and offline reproduction preserve
the API evidence; the mixed population does not establish code-only latency
or billed runner cost.

The [janitor report acceptance audit](ci-evidence/2026-10-22-janitor-report-disposition.md)
maps the original incident criteria to hosted evidence and continuing owners.
Global capacity and useful-seed durability remain tracked in #1801; the report
does not certify the broader CI backlog as complete.

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
  (`rust/tools/linux-validate/run.py`) drives Docker directly, so concurrent
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

Flatpak's pinned builder can restore cumulative ccache statistics along with
the build state. Those reports carry `counter_scope: restored-cumulative`
and `job_delta_available: false`; their raw counts are preserved, without
inventing a baseline after the action's final restore. Do not present them
as compiler hits from that job alone.

The existing full Ubuntu app lane captures a native zccache session journal
to diagnose misses such as the observed 137 cold OFF calls. Its IPC endpoint
and cache stay inside that job's workspace. Only a bounded, sanitized JSON
summary is uploaded as `native-cache-ubuntu-<mode>-<run>` for two days:
workspace-relative source candidates, outcomes, known miss categories, diff
field names and latency counters. Raw arguments, environment values, raw
journals and diff values are not uploaded. Missing, malformed or truncated
evidence is marked explicitly. Diagnostics are non-blocking; compilation and
integrity tests retain their normal failure behavior. This full-build lane
does not add work to PR or merge-group admission.

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

### Host Ubuntu package acquisition (#1269)

The existing Fast Gate, Qt frontend, full Ubuntu app and both Flatpak
saved-profile runtime jobs run
`sudo -E "$(command -v uv)" run --no-project ci/host_apt_mirrors.py` before
package work (the jobs that reach it before any other uv step add a
`setup-uv` step first, since the helper is Python; CI logic lives in Python,
not shell -- GEN-005, #962). It recognizes the
runner-image mirror list with Azure HTTP priority 1 and Ubuntu archive HTTPS
priority 2, then swaps those priorities. All URLs and security fallback remain
present. Missing, linked, duplicate or unfamiliar layouts keep their original
configuration; an actual rewrite failure fails the step. The helper needs no
download of its own beyond the uv setup the step already has.

Fast Gate, Qt and both runtime jobs use command-line acquire bounds of one retry and 15-second
HTTP/HTTPS timeouts, on both update and installation. Package lists, trust
checks, job limits and required tests remain unchanged. The motivation is the
observed Qt setup in run 39788036193: 104 MB took 19m25s with 347 ignored fetch
records before successful fallback transfers. Hosted outcomes must still be
measured; this observation is not a controlled speedup estimate or proof of
the underlying network fault.

### Optional Windows cache budget experiment (#1269)

Manual `ci.yml` dispatches accept `native-cache-probe` (default `false`).
Enabling it preserves the full dispatch validation and adds four clean full
Windows rebuilds per architecture after the normal app build. Use it only for
a planned cache experiment; PRs, queue runs, schedules and releases do not run
these additional rebuilds. No production cache limit changes automatically.

`ci/native_cache_probe.py` copies the completed build's compiler cache into
four isolated directories and compares `2G`, `350M`, `350M`, `2G` in that order.
Each trial starts from the same seed, explicitly cleans its own cache to its
limit, resets counters and invokes the existing Windows build preset with
`--clean-first`. The original cache and its publication policy are unchanged.
Four copies plus 1 GiB of free disk are required; insufficient disk or a failed
rebuild fails the experiment and preserves a failure report. Only successful
rebuilds are timing samples.

Each `native-cache-budget-<architecture>-<run>` artifact contains `report.json`
with source/image, commands, preparation and rebuild times, filesystem bytes,
and literal ccache statistics/version plus machine-readable counters. The
probe verifies the effective cache directory and rejects rebuilds with no
cacheable C/C++ calls. The pinned ccache supports these inspection commands
in its [manual](https://ccache.dev/manual/4.14.1.html). Artifacts expire after two days;
preserve useful results in `docs/ci-evidence/` before expiry. Filesystem bytes
are not compressed GitHub archive sizes. Two samples per size from one runner
do not establish representative-day latency, repository retention, or a
universal size recommendation. Compare compile time and actual hits/misses
before deciding whether to change the production limit.

### Optional native release-cache build qualification (#1269)

Manual `ci.yml` dispatches accept `native-cache-reuse-source` (default empty).
Set it to a complete producer SHA only for a planned master qualification.
It uses the existing four native app builds, with no extra compiler unit or
test executable. Normal full-dispatch coverage runs unless the explicit
measurement-only mode below is selected. Do not combine
it with native transport publication or the Windows budget rebuild probe.

Each native runner captures its own compiler/SDK/configuration identity and
requires an exact-source release-cache hit with that profile before building.
The restored directory replaces the Actions compiler cache for this run;
neither compiler-cache save path publishes experimental state. A missing or
mismatched profile fails qualification rather than becoming a hit. Tokens
remain read-only in the native app jobs.

`ci/native_cache_freshness.py` temporarily inserts a source/run-specific marker
into an existing frontend log literal, then checks for that marker in the
installed main executable after the real build. It records source and binary
hashes. The always-run cleanup restores the exact original source bytes and
refuses to overwrite an unrelated edit. Packaging is disabled for these builds.
The overlay is experimental evidence, not a shipped source change or an app
runtime test. `native-build-reuse-<platform>-<architecture>-<run>` artifacts
retain identity, restoration, overlay, binary verification and cleanup reports
for one day; preserve useful results before expiry.

Archive restoration plus a fresh binary still does not prove compiler reuse
or a net performance benefit. Retain the native compiler statistics, actual
build/restore/setup times, failures and cold conditions, and compare costs
against an appropriate control before adopting a backend. Routine PRs,
queue runs, pushes, schedules and releases leave this input empty.

#### Restricting a planned native measurement (#1801)

`native-cache-measurement-only` defaults to false. Set it only for a trusted
`obs-rust/obs-studio` master `workflow_dispatch`, together with exactly one
existing native protocol: `native-cache-transport-probe=true` or a complete
`native-cache-reuse-source` producer SHA. An explicit full/release gate,
Windows budget probe, Flatpak option, runtime-reuse request or Tier 2 setup
probe conflicts with this mode and fails input validation. Conflicting
standalone Flatpak/setup jobs cannot start while native-only mode is set.

The mode retains both Windows architectures and both macOS targets, their
existing identity/integrity checks, actual builds and reuse-source freshness
overlays, requested transport publication/consumers, formatting, Fast Gate,
data validation and required-result aggregation. It omits unrelated Ubuntu,
Flatpak, cross-lint, native Rust/Tier 2 and Qt matrices. No job or workflow is
added. The existing experimental publication permission and exact-source/profile
requirements remain unchanged.

This is a scoped performance experiment, **not full-platform validation** or
backend adoption. Routine PRs, merge groups, master pushes, schedules, releases
and ordinary dispatches retain their selection. Check for an existing manual
master run before dispatch: the shared concurrency group can cancel it. State
the missing evidence, expected runner/storage cost and sample scope first;
preserve failures and mismatches, and do not repeat a whole workflow merely to
get green. No native run is needed to test the routing fixtures themselves.

For a separately planned producer or consumer, respectively:

```sh
gh workflow run ci.yml --repo obs-rust/obs-studio --ref master -f native-cache-measurement-only=true -f native-cache-transport-probe=true
gh workflow run ci.yml --repo obs-rust/obs-studio --ref master -f native-cache-measurement-only=true -f native-cache-reuse-source=COMPLETE_PRODUCER_SHA
```

These commands remain opt-in. Changing routing alone demonstrates no cache
speedup, successful transfer, source freshness or global capacity solution.

The [first native build-reuse attempt](ci-evidence/2026-10-22-native-cache-build-reuse.md)
failed qualification: Windows profiles were cold, and successful macOS app
builds lacked installed-binary freshness proof. Its retained evidence includes
the final failed gate and successful compatibility and protocol checks.

## Tier 2 execution and Fast Gate overlap

The complete OFF/ON test jobs start after successful Tier 2 compilation and
scope/proof checks, without waiting for Fast Gate. No artifact from Fast Gate
is consumed by these jobs. Fast Gate remains a required job in the reusable
workflow, and its failure/cancellation still makes the caller's required gate
fail even when both CTest suites pass. A missing/failed build or workflow
cancellation does not start tests. Local Rust-only attestation can still skip
Fast Gate without covering Tier 2.

This removes a measured dependency wait: in the October 22 baseline, four
of nine successful PR runs and four of nine successful queue runs with both
jobs finished Fast Gate after the build, by up to 133/127 seconds. The median
opportunity across all nine in each event was zero. These are upper bounds
on the affected Linux path, not universal gate savings; native platform work
can still determine the final gate. Independent tests can consume runner time
on a change whose Fast Gate later fails. Record that extra failed-run cost
alongside successful latency during the #1694 post-rollout day.

## Linux Rust execution

The local gate's Linux Rust test commands use a checksum-pinned nextest
0.9.143 binary to schedule the existing category tests globally, with at most
four concurrent tests. The tool is installed in a local cache only when scope
selects Rust tests; there is no new Actions-cache writer. Other hosts keep the
existing Cargo runner. All commands still go through `soldr cargo`, with the
existing Rust wrapper bypass.

Nextest executes the same eight category binaries, not another set of test
targets. Cargo still runs documentation tests separately, including after a
unit-suite failure; neither failure can produce a passing lane. Scope applies
to both commands, and no retries mask a failure. The explicit
`ci/nextest.toml` configuration and runner installer are attested inputs and
proof surfaces. Workspace execution must discover tests; a scoped crate with
no test target retains Cargo's successful empty-suite behavior.

See the [experiment and limits](ci-speed-2026-10-22.md#rust-execution-schedule-across-existing-binaries)
for exact coverage and local timings. Native Windows/macOS workflow commands,
C tests, OFF/ON execution, source assertions and Rust compiler/cache settings
are unchanged.

### Rust timing observations in CI reports

`ci/cache_metrics.py --include-logs` records nextest and Cargo doctests as
separate command observations. It retains Cargo build/link summaries, time to
the first execution marker, the observed output span and nextest's explicitly
reported execution duration. These are distinct from full setup or process
runtime; missing markers remain unknown. Nextest passed/failed/skipped/not-run
counts are kept as reported, without inventing absent zeros or adding skipped
tests to `tests_run`. Separate Cargo result summaries retain doctest coverage.
The regression evidence includes a parsed real hosted run in
`docs/ci-evidence/2026-10-22-nextest-log-observations.json`.
When recollecting into the same output directory, unchanged run metadata retains
its recorded historical comparison base. Later PR rebases or base-branch updates
must not silently reclassify a cached run. Updated run metadata still invalidates
the cache and fetches current job/attempt details.


### Manual Flatpak measurement

Use the existing manual workflow to observe the complete Flatpak job without
launching unrelated native application, Qt, Tier 2 and cross-lint matrices:

```sh
gh workflow run ci.yml --repo obs-rust/obs-studio --ref master -f flatpak-measurement-only=true
```

`flatpak-measurement-only` defaults to false and is accepted only for
`workflow_dispatch`. Ordinary PR, merge-group, push, schedule and release
selection is preserved. Format/Fast Gate remain, and the entire Flatpak job
is unchanged: setup, cache restore, compilation, debug processing, export,
validators, pruning, required publication and cleanup all stay in its clock.
Keep other probe inputs off for a routine warm observation. A cold observation
also passes `-f flatpak-cold-build=true`; optional Flatpak qualification probes
remain separate extra work when explicitly requested.

This mode saves measurement runner time and waiting on unrelated jobs. Do not
count those omissions as a Flatpak build speedup. Record full job/source/cache
identities, failures and costs; do not restart a live observation on API lag.
Manual master dispatches share their existing cancellation group, so wait for
the entire preceding manual master workflow to finish before starting another.

### Flatpak cache retention after migration (#1801)

The production build restores one projected Actions archive containing
`.flatpak-builder/cache` and `.flatpak-builder/ccache`. A miss builds cold;
the old whole-builder and standalone compiler migration restores are retired.
Current builds reset counters after restoration and still execute the full
build, export and validators before trusted publication.

The obsolete standalone compiler family no longer reserves 200 MiB. The
[saved policy replay](ci-evidence/2026-10-22-flatpak-legacy-retirement.md)
records the declaration reduction and unchanged deletion plans. This snapshot
has no standalone entries to reclaim; global overcommit and native architecture
retention remain open in #1801.

The [allocation and version-retention follow-up](ci-evidence/2026-10-22-cache-allocation-followup.md)
maps all 25 remaining families, records overlapping publications and an actual
registry deletion across different host/container Actions versions, and links
the existing upstream identity defect. Its portable replay separates observed
deletions from hypothetical plans; global allocation remains unresolved.
