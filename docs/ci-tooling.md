# CI and build tooling: how not to drown the machines

> The `docs/ci-evidence/` snapshots linked below are retired from `master`
> (#2686); each link resolves to the commit that last held the file
> (`c2af3f9c3`), and the commands quoted below name those paths as they were
> then.

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
   with a valid signature on final HEAD. Only complete selected client gates
   plus verified-member identity grant fast admission; otherwise real PR checks run.
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
locally). Pinned soldr 0.9.54 defaults to the managed wrapper without this
override, and [zccache#1943](https://github.com/zackees/zccache/issues/1943)
remains unresolved. This safeguard preserves standalone C/C++ compiler
caching and soldr cook/target archives. All seven setup-soldr callsites also
set `build-cache: false`: the full-run audit observed no embedded session
across all eight expanded build lanes, despite transferring compiler-store
archives (42.9 MB and 1.1–2.2 s in the inspected Linux host jobs). This input
removes embedded compiler-store transport; it preserves cook, registry and
toolchain layers (the Tier 2 builds set `prebuild-deps: none`: Linux compiles Rust only in
Corrosion's own target dir, #3027, and macOS/Windows only there and in
`--target <triple>` build trees, #3034). The unused `setup-soldr:build-cache` retention family is
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
older Soldr cannot match the new base key or delta restore prefixes.
The pinned setup-soldr action defaults to Soldr 0.9.52; the workflows name the
version explicitly. See the [probe and limits](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-20-soldr-source-mtime.md).

For opt-in Rust compiler-cache qualification, run
`uv run ci/cache_source_probe.py --directory build_ci_audit/source-probe --soldr /absolute/path/to/soldr --expected-version X.Y.Z --timeout 120`.
Use a new directory for each run. The probe records the selected executable,
requires an unchanged cache hit, then checks that a same-size/same-mtime
`compile_error!` fails through both the cache and an independent uncached
control. It disables porcelain output so diagnostics remain visible and keeps
partial logs and `safe: false` results on timeouts. A passing Rust probe alone
does not qualify the native GCC rapid-edit case required by #3437; keep the
compiler-cache bypass until every required freshness check passes.

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
wrapper bypass above. `SOLDR_VERDICTS=off` forces a run; Build and Format set it for merge groups. The gate's source
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

The pinned soldr 0.9.54 embeds zccache 1.15.14, including the wrapper
selected by the optional `ENABLE_ZCCACHE` CMake module. Updating the standalone
installer alone does not update that embedded wrapper. Matching the versions
does not establish Rust compiler-cache correctness: the Rust wrapper bypass
above remains enabled.

Hosted Linux Tier 2 wraps its build in `ci/tier2_front_cache.py` (#4988).
A checksum-pinned ccache 4.14.3 binary provides a fresh, job-local 512M front
cache with `CCACHE_PREFIX=zccache`. This shares identical private-fixture C
compiles with different output paths while retaining zccache for misses. The
pinned zccache salts user depfile keys with output/target arguments; a local
24-output probe reported 24 cold calls versus 1 cold call and 23 front hits.
Both caches still validate headers and compiler flags. No sloppiness is enabled;
the front cache has its own empty configuration and is never uploaded. The
Dell's explicit empty launcher bypasses the helper. Rust caching, all test
assertions, sanitizer flags and OFF/ON suites retain their existing policy.
The expected hosted saving is an estimate until a completed job measures it.

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
- **Output:** objects only by default; it compiles and links nothing. `-DOBS_CROSS_LINK=ON` adds a shared libobs (`obs.dll` / `obs.dylib`) per alias, from the same sources the shipped hybrid links (including the `libobs-rust` staticlib, `deps/libcaption` and the generated version/core-module objects), against the prebuilt `.deps` import libraries. It stays opt-in because the link needs those inputs and a working cross linker, which not every host has.

```sh
cmake -S cmake/cross -B build_cross_win-x64 -G Ninja -DOBS_CROSS_TARGET=win-x64
cmake --build build_cross_win-x64
# link a real libobs (DLL/dylib) for the alias:
cmake -S cmake/cross -B build_cross_win-x64 -DOBS_CROSS_LINK=ON
cmake --build build_cross_win-x64 --target libobs-cross-shared
# or from a configured OBS build:
cmake --build build --target cross-win-x64      # cross-mac-arm64, cross-mac-x64, cross-all
```

- **Only Docker on the host:** `uv run --script cmake/cross/docker.py mac-arm64 win-x64` runs the same check in a container (through `uv run --script ci/docker_task.py`).
  - The checkout is a copy-on-write overlay at `/src`.
  - Writable caches are named volumes the next run reattaches: `/build` per checkout; soldr's LLVM and SDKs and the prebuilt deps shared.
  - `--clean` starts the build volume over.
- **Warnings** are reported; `-DOBS_CROSS_WERROR=ON` makes them errors.
  The [dependency warning boundary](../BUILD.md#dependency-warning-boundary-decision-of-2026-11-06-2625)
  exempts third-party headers through SYSTEM includes while keeping OBS headers
  and source strict. `ci/test_cross_conversion_ratchet.py` enforces it in
  `ci.yml`'s required `Format 🔍 / checks` job (`ci/ Self-Tests 🧪`), not in the
  external `zackees/ci.yml` repository. No additional workflow step is needed.
- **Getting soldr:** `cmake/common/soldr.cmake` uses soldr from `PATH`, or downloads the version CI pins and checks its SHA256 (`ci/test_soldr_pin.py` keeps that pin equal to the workflows').
- **Finding the toolchain:** `cmake/common/soldr.cmake` asks soldr for it (`soldr cc --target <triple> --print-cc/--print-cxx/--print-linker/--print-sysroot/--print-ar/--print-rc`), so a soldr layout change no longer breaks the check (`ci/test_soldr_cross_toolchain.py`).
- **In PR CI:** the `Cross Compile 🧰` job in `build-project.yaml` runs it, compile-only, for the aliases `ci/ci_scope.py` selects (`cross_targets`).
  It runs when a change touches a Windows or macOS file in libobs or a component the cross `CMakeLists.txt` names by `DIR`, or the check itself (`cmake/cross/`, `cmake/common/soldr.cmake`); full runs build both (#1294).
  The frontend's Windows and macOS Qt groups are in it (#2408): the target's prebuilt Qt6 headers, with `ui_*.h` from the pinned Linux `uic`; moc is skipped, since nothing links.
- **No `win-arm64`:** `util/threading-windows.h` uses MSVC-only ARM64 intrinsics that clang lacks.
- **Cost, measured 2026-10-31:** a cold cross build *with* the link — a fresh build tree, `ZCCACHE_DISABLE=1`, including that alias's `libobs-rust` staticlib — took `win-x64` 32 s / 41 s, `mac-arm64` 117 s / 32 s and `mac-x64` 134 s / 26 s (two samples each, `-j 4` on a 16-thread host shared with other worker checkouts; the slower sample in each pair is the one that also rebuilt 14–18 workspace crates, and the rest of the spread is co-tenant load). The native `Build OBS-Studio-Rust` steps it stands in for are 12m25 (Windows x64), 8m01 (macOS arm64) and 7m50 (macOS x86_64) in release run 41007587429; signing and packaging stay native.

#### Tier 2 pre-queue links (#5189)

The Linux-hosted Tier 2 Link Check matrix links hybrid libobs and every
configured Rust-static cmocka executable independently for Windows MSVC x86_64 and Darwin arm64
when `LANES["cmocka_link"]` selects the
change. This includes libobs, its linked crates, obs-build registration/support,
cmocka, Cargo inputs and the check's own implementation. Unrelated changes keep
the eight Rust category links in the independent Windows Link Check. The same check runs on PRs and merge groups;
selected cmocka links also run on fully attested member PRs because existing
client proofs do not attest this CMake pass. Category-only work keeps its
existing admission policy.

```sh
uv run --no-project --with cmake==3.31.10 --with ninja==1.13.0 ci/cmocka_link_check.py
uv run --no-project --with cmake==3.31.10 --with ninja==1.13.0 ci/cmocka_link_check.py --alias mac-arm64
```

The root CMake cross-link mode includes the unchanged cmocka graph and deferred
support fragments. CMake's file-api discovers executable targets after host
conditions have been evaluated, including consumers without a `_rust` suffix.
Imported Corrosion archives are discovered through generated link arguments as
well as target dependencies; an empty Rust inventory fails. The check uses
`RelWithDebInfo`, matching native Tier 2. Windows static consumers retain the
core's headers and demand-linked real C support without its full DLL import
library. The check rejects that import/static mix even if LLD accepts a link
that native MSVC LINK would reject (#5246). The check builds
against real target cmocka and SDK libraries, never executes target programs,
and saves `link-targets.json` in its build tree. It always refreshes the Rust
archive through soldr so source-only edits cannot reuse the cross project's
manifest-only custom-command result. Ninja uses the process CPU budget, capped at
four workers. Each alias uses a fresh 512M ccache from the checksum-pinned
`ci/tier2_front_cache.py` installer to share identical support-file compiles
within that pass. No cache is uploaded, no relaxed policy is inherited, and
the cache never stores link results. The matrix disables fail-fast so one failing alias cannot
cancel the other. `ci/gate_jobs.py` requires each selected alias on fallback
PRs and merge groups. Fully attested PRs defer both aliases to the merge queue.
The jobs retain the 30-minute deadline and existing Linux heavy-pool routing.
Windows Rust tests run on Linux in PRs and merge groups (#5845): job
`rust-tests-windows-wine` cross-builds the `x86_64-pc-windows-msvc` tests and
`ci/windows_wine_tests.py` runs them under Wine through `ci/wine-runner.sh`.
The runner is headless (no `DISPLAY`/`WAYLAND_DISPLAY`, no Mono/Gecko/winedbg/
winemenubuilder, crash dialog and AeDebug off) and creates its prefix itself;
run Wine locally only through it. Native `rust-tests-windows` runs on schedule,
dispatch, release and master push.
The local `ci/windows_link_check.py` default still runs categories and both
cmocka aliases; CI sets `OBS_CMOCKA_LINK=false` only on the category job.
Pinned CMake 3.31.10/Ninja 1.13.0 uv wheels avoid apt setup on this path
(#5224). CMake 3.31 uses clang-cl `/showIncludes`; CMake 4.3
forwards dependency flags that the pinned ccache rejects as uncacheable.
Darwin uses the production hidden-symbol list and the Rust crate's existing SDK
framework imports. Library discovery stays in the Apple SDK and target deps;
host Linux libraries cannot satisfy Darwin probes. A successful link does not
prove runtime recording/allocator behavior or every binding/order error whose
final link succeeds.

Earlier Release measurements before #5246's complete inventory and static-only
Windows support (four Ninja jobs on a shared Linux worker, 2026-11-19):

| Target | Fresh CMake/Corrosion tree | Warm tree |
|---|---:|---:|
| Windows MSVC x86_64 | 367.80 s | 82.49 s |
| Darwin arm64 | 418.54 s | 73.69 s |

SDKs, prebuilt target dependencies and the Cargo registry were already installed;
these are build-tree cold timings, excluding first-host downloads. The original
eight Windows category binaries also linked (128.53 s with partial Cargo reuse).
These local timings are not a hosted critical-path guarantee.

Standalone Rust-static support is selected for every consumer, including tests
that also link shared libobs or inherit Rust through an interface target.
`test/cmocka/build/rust-static-support.json` owns the real private adapter members
for both CMake and Cargo-native fixtures. Linux resolves Rust and support as a
rescan group so newly selected members can call back in either direction.
Fixture-owned adapters keep their compile definitions and take precedence over
the shared fallback members.
Add new shared adapters there as separate
lazy archive members; fixture-owned definitions retain precedence. CMake inherits
the native owner (`libobs-objects`) link interface rather than maintaining host
libraries per test. Cargo's production Rust-archive dependency carries its native
package interface. Both paths put Rust before MSVC import providers so allocator
and parser symbols are selected consistently; ELF/Darwin retain the fixture's
declared DSO binding order. Link checks prove linking only;
native runtime checks still own recording callbacks and allocator behavior.

#### macOS Rust execution artifacts (#5008)

The maintainer chose **STRICT** on 2026-11-17: nothing compiles on hosted
macOS runners. The Rust-test producer runs on Linux with soldr's LLVM and Apple
SDK targeting `aarch64-apple-darwin`; macOS loads and runs its artifacts. This
also applies to obs-build's native-build and Qt integration contracts. Compiler
discovery assertions apply to the Linux-hosted cross toolchain targeting Darwin;
their load and execution assertions still run on macOS. Keep every assertion,
including profile switches, source/flag mutations and unity/PCH rebuild decisions.

The Rust-test bundle is prepared by `ci/macos_test_artifact.py`:

```sh
uv run --no-project ci/macos_test_artifact.py build --output build_macos_tests
# PR selection, identical to local_gate.test_diff's affected-crate selection:
uv run --no-project ci/macos_test_artifact.py build --output build_macos_tests --base BASE
# Extract the complete bundle into the consumer checkout root first:
uv run --no-project ci/macos_test_artifact.py run --manifest manifest.json
```

The producer links the category tests and helpers with `soldr cargo test
--no-run`, verifies arm64 Mach-O executable headers and every ad-hoc SHA256
CodeDirectory page, and retains build-script output. It then compiles the same
selected crates' doctests with `soldr cargo test --doc --target
aarch64-apple-darwin`. Stable rustdoc `--test-runtool` copies each temporary
executable before cleanup. The SDK-aware linker comes from `soldr cc --target
aarch64-apple-darwin --print-linker`: Cargo's target rustflags alone do not
configure rustdoc's test linker. No nightly flags or `RUSTC_BOOTSTRAP` are needed.
The producer disables verdict replay so a previous doctest verdict cannot hide
missing runtime artifacts.

Rustdoc checks `compile_fail` and `no_run` cases on Linux. The retained 2024
merged harness checks runnable cases, including `should_panic`, on macOS. A
standalone `should_panic` case that rustdoc cannot merge fails preparation rather
than silently passing without execution. The manifest marks doctests complete
only after the entire compilation command succeeds. A producer success proves
compilation and compile-fail assertions; execution is proven only by the consumer.

Build and execution require **the same absolute, real checkout root**, including
the target directory. `ci/macos_test_workspace.py place` moves both real checkouts
to `/opt/obs-ci-<run-id>-<attempt>/src` before setup or execution, leaving the
original Actions workspace as an alias. `/opt` resolves identically on both hosts;
Darwin's `/tmp` resolves through `/private/tmp`. Placement supports separate runner
mounts and rejects a symlinked destination parent. The always-run restore step
returns the checkout before action post steps, retaining caches without leaving
large relocated trees on the Dell. Rust embeds `CARGO_MANIFEST_DIR` and `CARGO_BIN_EXE_*`;
changing environment variables after compilation cannot relocate them. The runner
rejects a mismatched root. The manifest retains package cwd, Cargo package/build
environment, test arguments and helper permissions, and hashes all transported
files. Missing artifacts, corrupt files, incomplete doctests and failed test
exits fail the consumer. Preparation requires an empty output directory.

The native-fixture slice adds `ci/macos_native_fixtures.py`. The producer calls
it whenever the Application category is selected; it uses that category's existing
Linux executable to prepare 17 Darwin fixture contracts. It covers profile and
assertion switches, shared-core/plugin dependencies, static archives and executable
links, explicit groups, independent C/C++ flags, Objective-C/Objective-C++ ARC and
manual groups, all unity/PCH combinations, transitive headers, source/flag changes,
Swift bridge/modules/headers and the independent CMake graph consumers. The
Windows resource/MSVC contracts and Linux-only runtime modules keep their existing
platform gates. Configuration-only fixtures use `project(... NONE)` or explicitly
disable compiler probing; they still run on macOS. Qt integration contracts use
the preparation helper below.

Independent native fixtures use a thread pool on the Linux producer, with at most
two workers by default (one on a single-CPU host). Set
`OBS_NATIVE_FIXTURE_WORKERS` to a positive integer to override this budget; it is
capped by `os.cpu_count()` and the fixture count. Use `1` for serial measurements.
Each fixture's ordered rebuild/mutation sequence stays in one subprocess. The
first observed failure reports that fixture's captured output, cancels pending
work and prevents a completed index; already running fixtures finish before exit.
The test inventory retains its existing order and signed-image paths are sorted
in fixture order, then by path, regardless of worker completion order.

For serial/parallel comparisons, reuse the same absolute bundle/work paths.
Existing toolchain nondeterminism affects snapshot blobs for `prefix.pch`
(serialized header metadata/order), `CMakeDetermineCompilerABI_C.bin` and
`CMakeDetermineCompilerABI_CXX.bin` (random `cmTC_*` probe names),
`CMakeConfigureLog.yaml` (random probe paths), `.ninja_deps`, `.ninja_log` and
`LastTest.log` (timestamps/timings), and `NativeSwiftFixture.swiftmodule`,
`NativeSwiftFixture.swiftsourceinfo` and `libfixture-swift.dylib` (temporary
Swift/PCH paths and derived debug data/UUID/signature). Those files' snapshot
`file`/`sha256` fields and corresponding `index.json` signed-image references
may differ. Compiler-command stdout also records CMake's elapsed configure time.
Normalize only these fields when comparing evidence; retain requests, inputs,
recipes, rebuild decisions, event order, destinations and all other file hashes.
Transport and replay always verify the original hashes and signatures, without
normalization.

Preparation discovers soldr's Darwin C/C++ drivers, archiver, LLVM linker and Apple
SDK on Linux. Swift uses an explicit `SWIFTC` override or the SHA256-pinned Linux
Swift 6.1.2 distribution, with soldr's LLVM tools for linking. Its Darwin bridge
still uses the same Apple SDK. A private tools directory also supplies `dsymutil`,
which Swift needs for debug builds and soldr's primary LLVM bundle omits. It uses
`DSYMUTIL`, an existing LLVM/host tool, or the checksum-pinned dsymutil from soldr's
LLVM-tools 20.1.7 payload. The fallback reuses soldr's verified installed tool or
downloads the pinned 474,401,547-byte archive and extracts only dsymutil; this is
producer setup cost, not consumer payload. Python 3.14 supplies zstd extraction.
Missing tools or invalid checksums fail preparation; no Swift assertion is skipped.
The dsymutil fallback persists under
`$RUNNER_TOOL_CACHE/obs-darwin-dsymutil/<archive-sha256>` (locally,
`target/macos-native-toolchains/obs-darwin-dsymutil/<archive-sha256>`).
Both archive and executable checksums must pass before atomic publication;
every reuse verifies the executable again. The archive is discarded, and
interrupted extraction cannot expose a cache hit. Separate log timings cover
the soldr driver/SDK queries, Swift version probe and dsymutil provisioning,
including its download, to distinguish network/setup cost from compilation.

The pinned Swift installation is reused across clean checkouts under
`$RUNNER_TOOL_CACHE/obs-darwin-swift/<archive-sha256>` when Actions supplies its
tool cache; local runs fall back to `target/macos-native-toolchains/` with the
same content key. Download and extraction happen in a private staging directory;
only a checksum-verified extraction containing the compiler is atomically
published. Interrupted installs cannot become cache hits. The archive is discarded
after installation. This caches only the immutable compiler distribution: every
fixture, mutation and snapshot is still prepared on every run. Logs distinguish
Swift installation/cache hits and time toolchain preparation, the host Application
category build, and fixture contracts with snapshots separately.
Compiler-discovery assertions therefore describe
the Linux-hosted Darwin toolchain. Every successful test writes a completed
manifest with `BUILD_HOST` (Linux), `BUILD_TARGET` (`aarch64-apple-darwin`), the
compiler recipes, input hashes, target-specific rebuild decisions, checksummed
snapshots of every intermediate variant and deferred runtime expectations.
Snapshot blobs are deduplicated by SHA256. A failed preparation never writes a
completed manifest or bundle index.

`OBS_NATIVE_FIXTURES` selects replay. The same test bodies assert the recorded
compiler results, restore the corresponding variant and really load or execute it
on macOS, including fresh-process loader probes and CTest's environment/working
directory checks. They reject missing/corrupt files, changed recipes, incompatible
host/target evidence and unconsumed events. Only compiler commands are replayed;
load/run assertions remain pending until the consumer succeeds. With neither
fixture environment variable set, local and Linux tests retain their native
compilation path. `OBS_NATIVE_FIXTURES_PREPARE` is the producer-only counterpart;
do not set both variables. Native manifests and signed arm64 dylibs, bundles and
executables are included in slice (a)'s final checksummed bundle.

`rust-tests-macos-build` in the existing build workflow prepares the full bundle
on Linux. It uses the existing per-job `mq-heavy-labels` routing with
`ubuntu-26.04` fallback. Both hosted and Dell producers upload the same gzip
archive and SHA256 sidecar; the job outputs its immutable artifact ID and real
checkout parent, so a macOS-only rerun retains the producer's original attempt; neither uses local runner handoff. The archive keeps
hidden build-script files and permissions. The consumer verifies the archive
before extracting, then verifies all manifest file checksums before any test.
PRs retain `rust_scope.affected` selection through `--base`; merge groups and
nondeferred events build the workspace. Qt preparation and `--require-qt` replay
share the existing `qt_sdk` scope. A missing Qt payload fails before execution.

Native macOS execution (`Rust Tests 🦀 (macos-26)`, `Rust Tier 2 (macos-26)`) no
longer runs on PRs or in the merge queue (#5846, #5843): they run on schedule,
workflow_dispatch, release and master push. The queue keeps the Linux macOS
artifact build, the aarch64-apple-darwin cross test link and the mac-arm64
Tier 2 link check.

The `Rust Tests 🦀 (macos-26)` check needs a successful producer and only
runs the bundle. Its setup consists of checkout for source/fixture assertions
and uv's prebuilt Python 3.14 for verification/helpers. CMake and CTest from the
runner image configure language-free projects and run existing tests. There is
no soldr setup, dependency cook, Qt SDK installation, compiler discovery or
compilation on this consumer. The published-SDK resolver and the four original
Qt graph/CMake contracts run on Linux against the Darwin SDK, recording their
target executions for macOS. Analysis outside #4999's queue replacement runs on
the Linux producer against the Darwin target. CI Gate requires the selected
producer and the unchanged macOS check; failed/missing/corrupt artifacts or failed
entries cannot satisfy it. Graph self-tests enforce these dependencies and the
absence of compile steps. Tier 2 macOS remains separate work in #5009.

The supplied hosted baseline spent 288–294 seconds compiling/linking Rust tests
and 42–61 seconds setting up soldr: **5.5–5.9 macOS minutes avoided per selected
job before transport overhead**, without double-counting #4999's analysis saving.
This excludes unmeasured native-fixture/Qt compilation savings. The 2026-11-17
integration run on shared Linux took 520.71 seconds for Rust/doctests/native
fixtures and 154.95 seconds for Qt preparation, **11.26 Linux minutes total**
before archive/upload/setup overhead. That run used existing caches but rebuilt
changed test harnesses and downloaded Qt SDKs; earlier slice timings (8.66 seconds
for warm Rust/doctests and 58.59 seconds for warm Qt contracts) describe different
cache states. These are local costs, not a hosted speedup or critical-path claim.
Only this PR's CI can prove the real common-root handoff, hosted macOS execution,
runner-image dependencies and net runner time on hosted and Dell routes.

The Qt infrastructure slice adds `ci/macos_qt_artifact.py`: matching published
Darwin Qt libraries and Linux moc/uic/rcc tools, both original native/CMake
contract consumers, signed arm64 output, and runtime entries in the same bundle.
See [the Qt contract inventory and producer command](ci-macos-qt-artifacts.md).
Preparation success leaves runtime pending; macOS must execute those entries.

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
`rust/tools/linux-validate/run.py` and `ci/docker_task.py` (through
`uv run --script`), the current stand-in
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
    Docker. It declares `group = "docker"`, its own heavy chain, so it runs
    beside the host lanes instead of after them (#2769); the gate's wall time
    is the longer chain, not the sum.
  - `macos-native` / `windows-native`: what the macOS and Windows Rust test
    and Tier 2 jobs run (tests, clippy, formatting, then the host's Tier 2
    validation, `rust/tools/tier2-validate/validate.sh|ps1`). Each is
    `optional` and exits 75 (`EX_TEMPFAIL`) on any other host, which `ci-lint`
    reports as "not applicable on this host": a Linux gate passes without
    attesting them, and only a Mac or Windows host can. Windows needs
    `vcpkg install cmocka:x64-windows` and `CMOCKA_PREFIX` (or
    `VCPKG_INSTALLATION_ROOT`), macOS needs `brew install cmocka`.
- **`ci/ci_scope.py`** maps changed paths to jobs, by path alone (#897). The
  Fast Gate always runs; Linux Tier 2 runs for the paths in its `LANES` row
  (table below). A changed file that belongs to a platform
  (`ci/platform_paths.py`: path tokens such as `win-capture/`,
  `platform-osx.mm`, `rust/*/src/platform/windows/`, or extensions such as
  `.m`, `.rc`) starts that platform's Rust tests and Tier 2. POSIX/Unix files
  (`*-posix.c`, `unix/`, and `libobs/util/platform-nix.c`) belong to Linux
  and macOS, so changing one starts macOS too. Core files carry no platform
  defines or OS headers (`ci/platform_ifdef_ratchet.py` enforces it, and
  fails a platform file that names another OS), so a core PR runs on
  Linux and Windows (#3637 step 2); file contents are never read. Windows runs
  the existing native Rust tests and Tier 2, including both C reference and
  hybrid libobs. macOS core PR coverage remains a separate rollout. A module
  whose name doesn't say its OSes
  (`coreaudio-encoder`, `obs-text`, ...) is listed in `ci/platform_paths.py`
  with its `PLATFORMS` from `plugins/CMakeLists.txt`; upstream paths are
  never renamed for this (#1051). `libobs/cmake/os-<os>.cmake` starts only
  its OS. Shared and so every platform:
  `.github/`, `cmake/common/`, `cmake/finders/`, the rest of `libobs/cmake/`, the root
  CMake files, crate-root `rust/<crate>/build.rs`, `platform/mod.rs`,
  `test/cmocka/CMakeLists.txt` and the C++
  cmocka tests with their `tests.d` registrations (#610). A master push adds the lanes a PR
  defers (Linux Tier 2, Qt, the workspace tests) and runs macOS/Windows
  Rust tests and Tier 2 for paths that select them **and for core code**
  (`libobs/`, `rust/`, Cargo files). This retains coverage for native core
  regressions (#2916, #2910). Merge groups execute selected heavy checks on the cumulative diff
  from the target branch's merge-base to the event head,
  including both native Rust/Tier 2 hosts for core/shared changes, selected
  platform paths, Linux Tier 2 OFF/ON, Qt and C/C++ cross work. Every non-docs
  group also runs Windows MSVC test linking and all-six-target clippy/Dylint:
  host, the two linked cross targets and three lint-only targets. Native app
  builds remain on nightly/manual/release events.
  CI Gate resolves the queue's full scope directly from the merge-group
  event's target-branch merge-base/head git diff, independently of reusable-workflow outputs.
  Missing event fields, unavailable commits and failed scope selection fail
  closed with named diagnostics. Cancelled jobs with no assigned runner and
  no steps are reported as runner-capacity conditions and still fail the gate;
   they are never converted to success. The existing AI rebase maintenance
   workflow runs `ci/mq_acquire_retry.py` in a separate `capacity-recovery` job
   on completed **Pull** `workflow_run` failures/cancellations, plus master push,
   dispatch and the :17/:47 schedule as a three-hour backstop. Recovery does not
   wait behind conflict resolution. It uses only default-branch code and API
   evidence, never the triggering head or its artifacts. Each event also scans
   recent completions, covering bursts where GitHub replaces a pending recovery
   job; the schedule remains the backstop for delayed or missing events.

   A PR run gets **one failed-jobs rerun** (attempt 1 only). A capacity-failed
   group that is still present at the exact tested SHA also gets one failed-jobs
   rerun, with fresh membership/PR-head checks. GitHub may mark that entry
   `UNMERGEABLE` even though the PR has no conflict: capacity classification uses
   exact annotations and CI Gate diagnostics, never that queue state. Run
   44351072142 (#5111, cancelled macOS job 134968894019, gate 134992644660) is
   the recorded example; it stayed queued until a manual removal/re-addition.
   A real PR conflict still blocks recovery. An ejected merge
   group gets a **fresh queue execution**, not a rerun of its departed synthetic
   head: the tool checks the latest removal belongs to that exact group, proves
   prior queue intent and an unchanged PR head, refuses conflicts, manual
   removals/disarms, holds and new queue transitions, and re-arms with
   `--match-head-commit`. A comment reserves one automatic retry per PR head
   before enqueueing; it shares the older ejection handler's retry marker.
   Each recovery still requires every selected job and CI Gate to pass.

   Both paths require exact hosted acquisition annotations and CI Gate capacity
   diagnostics for every unsuccessful job. Missing/unknown/incomplete evidence,
   independent gate failures and real test failures never authorize recovery.
   `github.token` (`actions: write`, `checks: read`, `contents: read`) suffices
   for PR and still-live group reruns. Re-arming ejected groups requires the
   token minted each run from the **obs-rust-ai-rebase** GitHub App
   (`vars.AI_REBASE_APP_ID`, `secrets.AI_REBASE_APP_PRIVATE_KEY`): enqueues
   with GITHUB_TOKEN do not trigger merge-group workflows. Without the App, PR
   and still-live group reruns still work; ejected-group re-arming emits a
   warning and leaves the PR alone. (Until 2026-11-22 this was a stored
   `AI_REBASE_TOKEN` secret, which was saved blank, so nothing was re-armed.)

   The older `ci/ai_rebase.py --requeue-ejected` remains a scheduled/push/dispatch
   backstop for broader proven infrastructure failures, now also guarded by
   the ai-rebase App. It inspects recent latest removals (three hours), binds the
   PR head via its timeline, classifies all associated workflow failures, and
   spends one per-head comment marker before re-arming. It skips armed, active,
   held or conflicting PRs. Runnerless cancellations are reserved exclusively
   for the strict capacity tool; the broader handler cannot bypass its refusal.
   Its broader policy does not run on `workflow_run`.
   The old capacity tool only scanned live queue heads at :17/:47/master pushes:
   once CI Gate ejected a group it was invisible, and PR runs were never scanned
   (#5115; merge-group runs 44306825758, 44318842478, 44323352759, 44329655339,
   44341056238; PR run 44318137015). A running CI workflow cannot rerun itself;
   external completed-run recovery keeps CI Gate's fail-closed behavior intact.
  Scope-policy edits (`ci/ci_scope.py`) also select Windows on PRs to exercise
  the routing on hosted CI. A failed or runnerless selected Windows job still
  blocks that PR. Selected queue jobs must also execute successfully; missing,
  skipped or unfinished expected matrix legs fail the gate. No required check
  is exempted and native master coverage remains in place.
  Evidence and the tradeoff are recorded in
  [the queue decision](ci-merge-group-scope.md).
  A master push builds no OBS app. Every platform and every app build runs on schedule,
  workflow_dispatch, tags, release branches and in the release gate; no PR
  path selects the full builds.
  Packaging and submodule moves are still compiled on Linux (touched targets)
  on the PR. `.github/` edits start no platform job.

### Path to lane table (fast gate)

A trusted complete ordinary member PR allocates Event Data, CI Mode and
CI Gate for compile/test admission. Independent governance/pre-work retains its
own event policy. Format, Swift, Validate Data and compile/test jobs skip before
runner allocation when their selected contract is satisfied. Fallback PRs run
Format/self-tests, Fast Gate and selected native/link/Qt/Tier 2 work. Every
non-docs fallback also runs all-six-target clippy/Dylint, including the three
extra lint-only targets. Fast Gate still compiles touched Linux targets on
fallback PRs; the complete member path defers those builds to the queue and
never claims a lint stamp proves a native target build.
`LANES` in `ci/ci_scope.py` selects the rest by path:

| Paths | Lane |
|---|---|
| everything | Fast Gate: 1) fmt, clippy, Dylint; 2) unit tests of the touched crates; 3) Linux build of touched `plugins/<x>`/`shared/<x>` targets (ENABLE_FRONTEND=OFF, no Qt) |
| `libobs/` (not platform-only files), crates linked into libobs, `test/cmocka/`, `deps/`, `rust/tools/linux-validate/`, Cargo files | Tier 2 Linux: compilation overlaps Fast Gate; OFF/ON tests wait for the Tier 2 build, with one complete suite per mode |
| core (`libobs/`, `rust/`, Cargo files), `ci/ci_scope.py` | Windows Rust tests and Tier 2 on PRs; both native platforms on master pushes and merge groups |
| `frontend/`, Qt-linked tests, `shared/qt`, the Qt job's own files | Qt Frontend Linux (the only Qt). Frontend platform files start no Windows/macOS Rust or Tier 2 job; Windows/macOS Qt lanes are a follow-up |
| files matching `/platform[/-](windows, macos, linux, posix)[/.]` (posix = linux+macos), legacy OS names, shared build files (`cmake/common`, `cmake/finders`, `libobs/cmake`, root CMake, crate-root `build.rs`, `platform/mod.rs`, C++ cmocka tests) | macOS / Windows Rust tests, Tier 2, cross lint |
| `build-aux/`, `.gitmodules`, submodules | compiled on Linux only; no full build from a PR |
| `docs/`, `.github/`, `ci/` | lint and the ci/ self-tests |

No path selects a sanitizer lane. ASan (and TSan, valgrind) run only in
`sanitizers.yaml` on its nightly schedule and on `workflow_dispatch`, never on a
pull request or in the merge queue (maintainer directive 2026-11-09; the
ratchet-PR `Sanitize 🧪` job of #2923 is gone). A diagnostic only asan reports
now surfaces in the nightly run; dispatch `sanitizers.yaml -f tools=asan` on a
separate ref to check a risky PR first.

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

Tier 2 uses one complete suite per mode on PRs and merge groups, each in its
own runner environment: two test jobs rather than six. Queue groups always use
these two complete suites, including when a caller requests full coverage.
Main pushes, nightly and release runs retain the suite plus two serial partitions
per backend.
The [runner-capacity measurement](ci-tier2-queue-capacity.md) explains the
earlier tradeoff: fewer allocations while retaining every test in that lane.
Deinterlacing retains
`RUN_SERIAL` because CI still dropped fields with bounded software-renderer
workers. The other nine former barriers
are removed: eight tests already own their resources, and XSHM now uses a
private Xvfb (serial fallback when unavailable). Linux tests pin
`LP_NUM_THREADS=1`; see the [concurrency audit](ci-serial-audit-2026-10-21.md).
The build job produces one tree for both backends. PR/queue test jobs run the
entire CTest registry without a shard selector, with up to eight binaries
concurrent within each suite; CTest honors `RUN_SERIAL`. Background partitions
together cover that same registry. A failed or canceled backend cannot publish a passing lane
proof. Timings have unique per-mode artifacts and merge on master; recorded
costs still order the longest tests first. Optional local sharding tools remain
available, including their complete-partition checks.

The change removes two repeated runner setups and artifact downloads. A full
suite can take longer than an individual shard, so fewer jobs alone do not
prove a shorter CI gate. Compare actual gate time, queue delay, coverage and
runner cost in the broader #1581 study before claiming a workflow speedup.

Unchanged lanes are cached by the gate, so a re-run after a small fix is fast.

Queue formatting starts after runner selection, alongside the Build reusable
workflow, rather than waiting for every heavy lane to finish (#4983). Queue
checks accept no client skips. Swift selection uses the whole group's diff
from its target-branch merge-base, including earlier entries; scope errors fail
the formatting job. The mutually exclusive PR call still waits for Build's
immutable-base admission outputs, preserving skips before runner allocation.
Both calls retain the executed `Format 🔍 / checks` and Swift check names,
and CI Gate waits for formatting as well as every selected build lane.
The measured baseline added 103–286 seconds of formatting execution after
heavy work in successful queue samples. Overlap can remove that tail, subject
to runner contention; hosted after-change timing remains to be measured.

### Lane proofs: historical diagnostics

Hosted input-key lane proofs and old `Local-Gate:` trailers remain readable
historical diagnostics. Neither suppresses real selected PR fallback or queue
execution. Publication jobs allocate only when an executed lane supplies a
nonempty new proof name; an attested skip cannot publish a fresh execution pass.
Compiler and dependency caches remain enabled. Command verdict replay is off on
PR and queue events so fallback analysis actually executes.

### CI attestation: skipping what's already proven

`./push` resolves the CI-pinned soldr wheel through `uv run --no-project --no-build --with soldr==<pin>`
(cached by uv), using `cmake/common/soldr.cmake`'s version, which self-tests keep equal
to every setup-soldr workflow input. Its directory leads PATH only during attestation;
your installed tools stay unchanged. Proof commands clear inherited `SOLDR_*`,
`RUSTUP_TOOLCHAIN`, Rust executable/wrapper selectors and their `CARGO_BUILD_*` equivalents,
so the repository toolchain and lane settings apply. To use an already installed copy,
set `PUSH_SOLDR=/absolute/path/to/soldr` (or `soldr.exe`); it must report the exact CI version.
Bootstrap/version failures use ordinary PR CI, or refuse with `PUSH_REQUIRE_ATTESTATION=1`.

CI Mode extracts `ci/pr_attestation.py` from the immutable PR BASE and runs it
through uv in isolation from head code. That executable loads its prooft source
pin, gate declarations and freshness/input policy from the same base git objects.
The untrusted head commit/tree/message is data. Workflow-call outputs carry that
one decision to Format/Swift and Validate Data before runners allocate. Event
Data selects Swift from immutable base/head names, so a skipped Format checks
job cannot hide Swift work. CI Gate independently reruns the base verifier and
requires every expected fallback job to have executed successfully. Missing CI
Mode or unexpectedly skipped/missing/cancelled/runnerless work fails admission.

The fast path requires a same-repository PR whose author AND authenticated
final HEAD signer are active obs-rust org members. GitHub must report that exact
commit signature valid. Authenticated public-membership checks require no extra organization permissions
for public resources ([GitHub API contract](https://docs.github.com/en/rest/orgs/members#check-public-organization-membership-for-a-user));
private membership uses the authenticated active-state endpoint when permitted; denial, unknown identities, API errors or verifier
errors mean real selected PR checks, without expanding token permissions.
`author_association` and collaborator write access are insufficient.

Evidence is self-reported, not signed execution proof: a trusted member can lie
about running a command. prooft checks freshness (24 hours), tree/parents or
base-declared input keys, provenance and each gate. Stamping amends the message:
**stamp first, then sign the final commit**. An old signature is never credit for
the amended object. Rebase, content edits, stale/wrong-tree/malformed records,
missing gates and edits to trust surfaces (`push`, `ci/**`, `.github/**`,
`local-gate.toml`, `ci-attestations.yml`, manifests/toolchain/config) run real CI.
Legacy-only and partial legacy evidence never earns complete admission.

Trust-surface rejection and the admission fallback scope selector use the same
merge-base-relative PR path set (`git diff BASE...HEAD`, with renames disabled).
Changes made only on master after the branch point, including changes brought
into the PR by merging master, are not attributed to the PR. The PR's own trust
edits still force fallback. Missing merge bases (including shallow or unrelated
history) and diff errors run real checks; an unknown scope selects every
admission check. This changes only path attribution: policy and verifier pins
still come from the exact immutable BASE, and evidence still binds HEAD. The
merge queue always builds and tests the actual combined result with its selected
heavy lanes, so interactions with newer master changes remain covered there.

The complete client contract includes full self-tests, Flatpak and Qt XML
validators, general lint, Linux x64 host unit/differential tests, and clippy AND
Dylint for exactly these targets (all through soldr):

- Linux x64: `x86_64-unknown-linux-gnu`
- Linux arm64: `aarch64-unknown-linux-gnu`
- Windows x64 MSVC: `x86_64-pc-windows-msvc`
- Windows arm64 MSVC: `aarch64-pc-windows-msvc`
- macOS x64: `x86_64-apple-darwin`
- macOS arm64: `aarch64-apple-darwin`

Swift formatting and compatibility/services JSON gates are additionally required
when the base-owned selector finds those files. A complete check can use its own
trusted gates; heavy native/link/Qt/Tier 2 work only defers under the full selected
contract and the base-owned `heavy-v1` queue placement policy. The existing Fast
Gate and main cross jobs also own native compilation/linking, so partial client
evidence conservatively runs them in full. Cross lint never proves native tests,
Tier 2 runtime or ABI/hardware/package behavior.

On merge groups with explicit cumulative `cross_lint` scope, the macOS Rust
Tests job leaves formatting to Fast Gate and Clippy to the required macOS
arm64 cross leg (#4999). That linked cross leg lints `--workspace --all-targets`,
including the category tests and C oracle; its Dylint scope stays unchanged.
CI Gate requires both replacement jobs to execute successfully on the merge
result. Missing/false scope, measurement-only modes and other events keep Darwin
analysis on the Linux artifact producer (#5008). Rust tests, runnable doctests and every Tier 2
pass still execute on macOS. The two inspected jobs spent 155–162 seconds in
native Clippy plus 8–11 seconds in formatting; this is the expected native
runner saving, pending hosted validation, with extra test/oracle analysis on
the existing Linux cross runner.

Outsiders need no client tools. `./push` warns and falls back if local tools or
signing are unavailable; `PUSH_REQUIRE_ATTESTATION=1` is the opt-in strict mode.
A complete stamp is required for this fast path, never for contributing RED PRs.
The implementation's own trust-surface PR must run real checks. A later ordinary
source PR is needed to observe verification-only allocation after this base policy
lands. Record actual job IDs/execution and runner wait separately for the member,
fallback and queue examples; fixtures prove routing, not measured speed gains.

## Before you push

`./push` runs prooft through `uv run --no-project --with` using the immutable
`PROOFT_SOURCE` pin in `ci/pr_attestation.py`, shared with the base-owned
verifier. The pin includes upstream commit `d08d58be`, which adds the `data`
and `swift` ecosystems declared in `ci-attestations.yml`. Released prooft
0.1.0 and 0.2.0 reject those gates before running a proof command; an older
prooft on `PATH` is therefore ignored. uv caches the pinned environment
without replacing the installed tool. A pin download or proof failure still
uses ordinary PR CI, or refuses publication with `PUSH_REQUIRE_ATTESTATION=1`.

For a focused CI/configuration bug fix, keep the RED and GREEN commits and
record the local failing and passing commands, but push them together when
the fix is ready. A separate remote RED run is not required. This avoids
spending runner time on an already-fixed failure; see
[coordination.md](rust-port/coordination.md#4-pull-requests).

`./push` uses gh's configured default repository as the PR base and resolves
Git's actual `origin` push URL, including a separate `pushurl`. Fork heads use
the destination owner's name; new PRs target `master`. An existing open PR for
the same destination and branch is reused, including a stacked PR. Configure
exactly one push destination on the same GitHub host as the base; ambiguous
destinations and repository-resolution errors stop before the push.
If the selected base is a fork, explicitly choose it with
`gh repo set-default OWNER/REPO` or `GH_REPO`; captured repository queries cannot
ask gh's interactive fork-selection question.

When stamping amends an already-published HEAD, `./push` uses an exact
`--force-with-lease=<branch>:<pre-stamp-sha>` only if the push destination
still names that pre-stamp commit and the amended commit preserves its tree
and parents. A concurrent remote update rejects the lease. New branches and
ordinary fast-forward updates use a plain push; divergent history is refused
by default. The publication message reports which case applies.

To publish intentionally rewritten history (for example, a rebase or a stacked
draft reset onto master), use `./push --replace-remote-head <old-sha>`, naming
the exact full SHA of the remote commit you intend to replace. Review the
commits being replaced first. `./push` still runs its proofs and signs the final
HEAD, then checks the actual `origin` push destination, including `pushurl`.
It refuses if that branch is absent or its head differs from `<old-sha>`.
Only a match enables `--force-with-lease=refs/heads/<branch>:<old-sha>`;
Git also rejects a concurrent update after the check. Abbreviated SHAs and
symbolic refs are rejected. No unconditional force option is supported.

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

## C++ lint report (frontend)

The Qt frontend has a C++ lint report (#5453), `ci/cpp_lint.py`:

- clang-tidy (`clang-tidy==22.1.8` through uvx) with the checks in
  `frontend/.clang-tidy`, on the unity groups that the change touches. A
  changed implementation file runs its group. A changed header runs the groups
  that include it (see the gate below). A change to the config, the script or
  the baseline runs every group.
- clazy (Ubuntu's `clazy` package, `clazy-standalone`): level 0 and 1 without
  `connect-by-name` and `qproperty-without-notify` (upstream's Designer
  patterns), on the same groups and in the same time budget as clang-tidy.
  The Qt lane installs it with `apt-get` only when there are groups to lint
  (about 80 MB with its LLVM); `./lint` uses it when it is on `PATH`.
- lizard (`lizard==1.26.1`): the cyclomatic complexity (CCN) and length (NLOC)
  of every new or changed function in the changed frontend files, with the
  values before the change.

**The gate** (#5453 step 3): `ci/cpp_lint_baseline.txt` holds the findings
master has, one sorted line per key with its count. A key has no line number,
no machine path and no number that moves with an edit, so moved code is not
new. In a PR or merge group the Qt lane fails when a key goes above its count
(`ci/cpp_lint.py --gate`, exit 3, the only exit code that fails the job); the
report names the baseline line. Fix the finding, or add its line only for a
finding you keep on purpose. A finding gone from a changed file is listed (lower
or delete its line) but does not fail. A tool that did not judge every group
(not available, a timeout, over the budget) or has another version than the
baseline records judges nothing. A change to the baseline, the config or the
script runs every group. `--update-baseline` rewrites the file from such a run.

In the Qt lane the build's own dependencies (`ninja -t deps`) map a changed
header to exactly the groups that include it. Without them (`./lint` with
another generator) the precompiled-header rule applies.

`./lint` stays report only.

- **Qt lane:** `ci/linux_qt_frontend.py` exports `compile_commands.json` and
  runs the report after the tests. The changed files come from the GitHub
  event: the PR's base, the merge group's base, or the push's `before`.
- **`./lint`:** runs the report when `OBS_CPP_LINT_BUILD` names a build dir
  with `compile_commands.json` (configure with
  `-DCMAKE_EXPORT_COMPILE_COMMANDS=ON` and the Ninja or Makefile generator,
  then build once). Otherwise it prints a skip line. The changed files are the
  changes since the merge-base with `origin/master`, including uncommitted and
  untracked files.
- **By hand:**
  `uv run --no-project ci/cpp_lint.py --build <dir> [--base <rev>]`.

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

The Tier 2 apt family has one master-push writer in `rust-linux-tier2-build`.
Its restore/save and the OFF/ON consumers use the same image/package-list key
and literal relative path `../../_temp/apt-archives/*.deb`. The default checkout
layout puts that directory under `RUNNER_TEMP` on host and container runners;
apt scratch stays outside the checkout. Actions also versions entries by
their input paths: an absolute `runner.temp` path changes between hosted and
Dell runners even with an identical visible key. The previous writer added a
trailing hyphen that the test consumers did not request. The first hosted master
build after this correction seeds the portable version; until then consumers
install normally. Only completed packages travel, and apt locks stay job-local.
Dell builds retain their marker-backed shared archive and skip remote saves.
The existing `tier2-apt` 400MB allowance and janitor policy are unchanged.

The app builds keep no Actions cache (phase A of #1801): the macOS Xcode
compilation cache, the Windows ccache, the Ubuntu zccache and the Flatpak
builder state held 5.71 GiB and served only nightly, dispatch and release
runs, while `deps`, `zccache-tier2` and `tier2-apt` serve every PR and queue
run. `ci/test_cache_family_budget.py` checks that the declared family maxima
fit in `[cache].budget`. The release-asset transport (#782) can bring the
app-build caches back outside the budget; its dispatch-only seed goes from
the app build to the transport job as a one-day artifact.

A branch dispatch of the `asan` pass restores only the archive master saved, so that archive has
its own family, `zccache-sanitize-asan`. `tsan` and `valgrind` keep no archive.
Master builds fresh and never restores: a restored archive kept the old entries
beside the new ones and outgrew the family. The nightly saves one, and a master
push calls `sanitizers.yaml` with `build_only` when `ci/asan_cache_state.py` finds
no archive or one older than 6 hours (#3501). A restored archive builds in about
4 minutes under 6 hours old and about 8 at 6-15 hours, against about 24 cold.

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
The [retention snapshot and offline executor reproduction](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-21-cache-retention.md)
record that churn separately from the still-open whole-budget allocation and
durability work. A current inventory below the global budget does not prove
that useful seeds survive the next nightly writer burst.

The [24-hour PR and merge-group cohort](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-22-ci-cache-day-cohort.md)
records gate latency, failures, cancellations and incomplete timing proxies
separately for each event. Its companion and offline reproduction preserve
the API evidence; the mixed population does not establish code-only latency
or billed runner cost.

The [janitor report acceptance audit](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-22-janitor-report-disposition.md)
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
- **Native lane capacity.** The heavy queue requires selected native runtime
  coverage, even with complete client cross-lint evidence. Runner shortages fail
  admission; they do not grant queue proof credit.
- **Warm PR caches.** setup-soldr restores but doesn't save on PRs, so a
  branch's first build depends on master's cache being warm.
- **Feed lessons back.** Problems found here, such as the stale zccache
  replay or the gate lock contention, belong in the tool's own repo, so every
  project using the protocol benefits.

## Cook profiles and post-job snapshots

All setup-soldr consumers declare their cook policy. Static-library and
release jobs cook release dependencies. Native test and cross-check jobs
use the dev profile; cross checks still install their target standard
libraries through the existing gate, and host cooking does not imply
that a cross-target dependency tree is prebuilt.

The Linux `cross-compile` job skips cooking (`prebuild-deps: none`) and keeps
`cargo-registry-cache: true` (#3767). Its default C/C++ compile-only check has
no Rust consumer; the optional `OBS_CROSS_LINK` path builds a cross-target
release staticlib, not host dev artifacts. Baseline PR run
[42310800738, job 128211841721](https://github.com/obs-rust/obs-studio/actions/runs/42310800738/job/128211841721)
restored an 18.7 MB host cook, loaded 714 files in about 13 seconds, then
cooked `--profile dev`; the subsequent compile-only check printed zero Rust
`Compiling` lines. Edits to `build-project.yaml` select both cross aliases so
PR CI validates this job's setup changes. The macOS/Windows app and release
jobs retain their existing cooks pending measured consumer evidence.

Fast Gate cooks the dev profile with `--tests`, on every event: its consumer
is `cargo test`, so a release cook there is never reused. Measured on ten Fast
Gate jobs on 2026-10-31 (#2661): the four jobs that still cooked `--release`
(push and merge-group core runs) then rebuilt the `test` profile from scratch,
1m09s-1m37s and 256-442 `Compiling` lines, while the six jobs cooking
`--profile dev -- --tests` finished that build warm with no profile line at
all (192 lines). The cook step cost the same either way, 11.3-15.6 s. The
older policy came from the local post-job benchmark (#1317), which timed the
cook step (57.259 s release, 36.170 s dev, 72.136 s without cook) and not the
consumer it feeds; core rebuild medians favored release but varied widely.
The action hashes profile flags into the cook cache key, so changing them
starts a new archive generation that master has to seed.

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

The Flatpak OBS build reports zccache hits, misses and uncacheable compiles
from its Ninja build session. Log evidence marks those counts
`counter_scope: job-session`. Zero hits only warn, unless the step runs with
`--expect-reuse` (the compiler-transfer consumer, or the
`flatpak-expect-compiler-reuse` dispatch input), where they fail the job
(#5788). A Flatpak zccache run ID cited here or in the migration note must
be recorded in `docs/evidence/zccache-flatpak.json`. Historical Flatpak ccache reports can
include cumulative restored statistics and carry
`counter_scope: restored-cumulative` and `job_delta_available: false`.

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

The summary retains numeric compiler `child_peak_rss_bytes`, sampled
`tree_peak_rss_bytes` and their source, exit codes and termination signals when
the journal supplies them. The child high-water mark can be just the compiler
driver; the sampled tree includes its compiler descendants. Neither measures
the cache daemon's RSS or the simultaneous host peak. Missing fields are unknown,
and requests lost before journaling have no completion measurement. It also
retains bounded, allowlisted lifecycle events from the job-owned cache, without
making daemon control requests. Missing death
events cannot distinguish SIGKILL, a crash, or a connection-handler failure.
In particular, run [41785304209](https://github.com/obs-rust/obs-studio/actions/runs/41785304209/job/126426654456)
retained only 34 completed misses and simultaneous client disconnects: it does
not establish OOM. Track the daemon investigation in
[zccache#2064](https://github.com/forge-ci/zccache/issues/2064); a retry succeeding
is not evidence that its cause was fixed.

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

The merge queue is the heavy gate (#4747, maintainer direction in #4743).
`ci/ci_scope.py` selects lanes from the merge-base of the event's target
`base_ref` and immutable `head_sha`, diffed against `head_sha`. The target is
fetched from origin, recovering full history for shallow checkouts. This includes
earlier queue entries even when `base_sha` names the preceding entry's head
(#4964). Unknown scope selects all work; CI Gate still rejects unresolved scope.
It runs Fast Gate merge-result Rust tests,
host formatting/clippy/Dylint and touched Linux targets, selected C/C++ cross
compilation, complete Linux Tier 2 OFF and ON suites, Qt, and selected native
Windows/macOS Rust tests and Tier 2. Shared/core changes select both native
hosts, matching core master-push coverage; platform paths retain their existing
selection. Every non-docs group links Windows MSVC test binaries in the
dedicated Windows Link Check and links macOS arm64 test binaries in the cross
matrix. Clippy and Dylint run for all six targets on the client or in PR
fallback, before queue entry. GNU-oracle linking remains in Fast Gate when
its inputs select it. Docs-only groups retain their Fast Gate skip.

Client trailers, hosted lane proofs and command verdict caches cannot suppress
selected queue execution. `SOLDR_VERDICTS=off` disables soldr verdict replay;
restored compiler/dependency artifacts remain allowed. PR coverage is retained
for fallback contributors; complete verified-member PRs now use base-owned admission.

`ci/gate_jobs.py` recomputes an expected execution manifest from that same exact
diff, with full checkout metadata, independently of reusable outputs. Each
selected matrix leg and both complete Tier 2 modes must finish successfully.
Missing, unexpectedly skipped, unfinished, cancelled, failed or runnerless jobs
reject admission. True out-of-scope jobs may skip. API/scope errors fail closed;
runner-capacity diagnostics remain errors, including unavailable native runners.
`CI Gate ✅` remains the sole required context. Only cache-heavy Linux jobs use selected
`MQ_RUNNER_LABELS` with hosted fallback; other jobs retain hosted runners.
Concurrency cancellation and independent cache maintenance are preserved.
Sanitizers, security scans, full app builds, packaged/hardware qualification and
release work retain their existing nightly/manual/release placement.

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

Packaged virtual-camera audio hardware qualification lives in the existing
`vcam-audio-hardware.yaml`: daily at `17 10 * * *` (10:17 UTC) and by manual
`workflow_dispatch`, with scheduled jobs restricted to `obs-rust/obs-studio`.
It never runs for PRs (including hardware harness/workflow edits) or
`merge_group`, and has no dependency on `CI Gate` or attestation shortcut.
Both Windows VB-Cable and macOS OBS Virtual Microphone jobs select the newest
successful scheduled master package, or the optional manual `build_run`.
Each summary records the package name and build run ID; this qualifies that
package, not the PR or merge-group tree. Package, driver and audio validation
failures remain visible, with bounded job timeouts and failure artifacts.
`ci/test_vcam_audio_hw_events.py` checks this routing and retained recording
checks without claiming an executed hardware pass.

### Opt-in merge-queue runner

Set the repository Actions variable `MQ_RUNNER_LABELS` to a JSON array such as
`["self-hosted","linux","x64","obs-mq"]` to route cache-heavy Linux jobs on `merge_group`
to online runners matching **all** those labels. The supported credential is the
GitHub App `obs-rust-mq-runner-probe`, installed on the `obs-rust` organization
with access to this repository. Give the App repository **Administration: read**
and organization **Self-hosted runners: read** permissions. Set the repository
Actions variable `MQ_PROBE_APP_ID` to its App ID (`5736334`) and the repository
Actions secret `MQ_PROBE_APP_PRIVATE_KEY` to its private key. The hosted selection
job creates a short-lived installation token only for `merge_group` when both
`MQ_RUNNER_LABELS` and `MQ_PROBE_APP_ID` are nonempty. Token creation is allowed
to fail; an absent App token falls back to the optional legacy
`MQ_RUNNER_STATUS_TOKEN` secret (a PAT with the same read permissions).
Both repository-level and organization-level runners are supported.
The `select-mq-runner` job calls `GET /repos/{repo}/actions/runners` and
`GET /orgs/{owner}/actions/runners` (owner is the repository prefix) before
selecting labels; matching capacity across the available lists qualifies according
to `MQ_RUNNER_MODE`. Its workflow token has **Actions: read** for the separate
run/job commitment probe; the runner-probe App needs no additional permission.
The `heavy-labels` output goes only to Build's `mq-heavy-labels` input. An organization API 403/404 (for example,
when the token lacks organization scope) emits a warning and keeps the repository
result. API failures in either scope warn without discarding a match in the other.
If App token creation fails and no legacy token is available, selection falls
back to hosted runners. Missing tokens, invalid label JSON, or no matching online
runner in either available list emit a warning and keep each job's original hosted label. The probe
checks at most five pages of 100 entries per list, with a ten-second timeout per
request and **210 attempts total**, shared by runner, run and job probes including
retries. This covers two runner scopes, two run statuses and ten active runs
(the ruleset's five build entries times two for regroups), each with up to five
pages and three attempts per page. Timeouts, connection failures, HTTP 5xx and 429 get at most two retries
with short jittered backoff; Retry-After (seconds or HTTP date) is capped at five
seconds. Other HTTP 4xx responses are never retried. A shared 45-second deadline
caps requests and backoff across the selection, well inside the job's two-minute
timeout; an expired deadline discards even an earlier matching scope.
Active-run pagination ends at a short page, rather than trusting a changing
`total_count` to end a full page. A count mismatch or duplicate ID retries the
traversal from page one, at most twice within the same request budget/deadline.
A traversal with consistent counts, or two consecutive short-ended traversals
with identical unique run IDs, completes the collection even if the count is
stale. Collections that keep changing or exceed the page limit select hosted;
incomplete job lists still select hosted. If the jobs budget runs out
after active runs are listed, each unprobed active run instead reserves the
required runner capacity (or its larger observed queued-job count) and one run slot.
Warnings name the failed probe step
and include validation reasons, including incomplete collections and limits.
An unset or empty variable skips the
probe steps. Outside `merge_group` the selection is empty unless member PR
routing is explicitly enabled below. Master pushes, manual/release runs,
and macOS/Windows jobs keep their current hosted runners. Linux matrix entries
retain their matrix and hosted fallback. Required job/check names are unchanged.
Rust Tier 2 Linux build, its OFF/ON test jobs, Rust Cross Lint
(two main targets plus the release-target job), and the Linux macOS Rust artifact
producer and Windows Link Check use that selection.
Build and tests share the selected pool so local handoff remains valid.
On merge groups, Event Data uses a bounded Forge build-runner reservation
when the pool is selected and a lease class is configured. With no ready
lease it runs hosted. Its full history checkout needs a build-sized runner. CI Mode uses the quick runner when configured and
skips PR-only admission steps. Both use hosted runners when their routing
inputs are absent. Format, Fast Gate, Cross Compile, Qt, app builds, proof
publication, queue cancellation and CI Gate use hosted runners.

On `merge_group` the selector job itself runs on the org's `forge-quick` admin
runners when the repository variable `FORGE_QUICK_LABELS` holds their labels
(`["self-hosted","forge-quick"]`), so a queue run's routing decision never waits
in the hosted queue (#5376). forge-quick runs one job per ephemeral container
with a 120-second wall clock, 256 MiB (its tmpfs workspace included), half a CPU,
no Docker and no LAN; the job therefore checks out `ci/` blobless and pins
`setup-uv` to the image's cached uv 0.13.7. PR, push and scheduled runs keep
`ubuntu-latest`. Kill switch: delete the variable
(`gh variable delete FORGE_QUICK_LABELS -R obs-rust/obs-studio`) and the next
run selects on hosted again.

#### Idle Dell capacity for member PRs (default off)

The maintainer approved same-repository member PR routing on 2026-11-19
(Refs #4839, #4896). Set the repository Actions variable `DELL_PR_ROUTING=on`
to opt in; unset, `off`, or any other value keeps PR jobs hosted. This change
does not set any repository variables. Forks, non-members, pending memberships,
API errors and `pull_request_target` events always stay hosted. Eligibility uses
the immutable `pull_request` event's head/base repository identities and author,
then the admission verifier's authenticated public/private active-membership
check against obs-rust. The hosted selector checks out the PR's BASE SHA and
extracts both `ci/select_mq_runner.py` and `ci/pr_attestation.py` with `git show`
into an isolated directory. It never imports the PR's routing code. The runner
probe App/token must be able to read membership; denied private membership uses
hosted without expanding permissions. Probe credentials remain on hosted runners.

`MQ_PR_RESERVE` keeps matching idle runners free for the queue, defaulting to
`MQ_RUNNER_DEMAND` (currently `6`). A PR must fit its entire required capacity
after queued commitments and this reserve are subtracted. Unset/empty uses the
default; a nonnegative integer overrides it, and invalid values select hosted.
PRs have a separate `MQ_PR_DEMAND`: unset/empty defaults to `13`, the complete
expanded PR job inventory, including both sequential Tier 2 phases. This count
is a **hard admission floor**, checked against every expanded job that can
receive Dell labels for `pull_request` in `build-project.yaml`. Selection does
not throttle or restrict jobs after admission: Fast Gate, Cross Compile, Qt,
link jobs and the existing heavy jobs can all receive those labels. A smaller
positive override is clamped to the full inventory (and logged); it cannot turn
that thirteen-job graph into a two-job allocation. Larger overrides reserve
more capacity. Invalid/nonpositive values use hosted. The effective PR demand
is the maximum of this inventory, `MQ_PR_DEMAND` and the MQ requirement, and is
also used for in-flight PR commitments. Update the inventory floor and its
regression whenever the workflow graph changes.

**Six-runner Dell:** keep `DELL_PR_ROUTING=off`, `MQ_PR_RESERVE=3` and
`MQ_PR_DEMAND=13`. There is no PR-demand setting that safely admits the current
thirteen-job graph with a three-runner queue reserve on six runners; at least
sixteen online/idle runners are needed. In particular, the reported
`MQ_PR_DEMAND=2`, `MQ_PR_RESERVE=3` configuration must select hosted. Useful PR
routing on six runners needs a separately bounded job subset or execution
limit before reducing the inventory floor. With reserve six, nineteen matching
idle runners are needed. `MQ_RUNNER_LABELS`, `MQ_RUNNER_MIN_IDLE`,
`MQ_RUNNER_DEMAND` and `MQ_RUNNER_MODE` retain their meanings.
No variable is changed by merging this default-off feature.

When enabled, both selectors account for live PR and queue job commitments.
Undecided queue runs have priority over every PR, regardless of creation time;
undecided PRs use oldest-first ordering. Commitments are weighted by each
event's required runner capacity, so a large PR cannot count as one small MQ
run. For MQ admission the budget is `MQ_RUNNER_MAX_RUNS * required`; PR admission
keeps the online-capacity limit and idle floor after subtracting the MQ reserve.
Queue admission does not subtract the PR reserve. This remains a bounded
snapshot, with the same API visibility and
simultaneous-selection limitations as queue admission, not a preemption system.
Already-running PR jobs finish normally. Turning the kill switch off stops new
PR routing; allow existing PR jobs to drain before relying on queue-only census.

Existing heavy Linux jobs accept the selected labels on PR events. Fast Gate,
Cross Compile and Qt Frontend Linux also accept them for PRs only; their queue
and other-event placement is unchanged. On PRs, Event Data, CI Mode and
lightweight admission/gate jobs stay hosted. macOS/Windows jobs keep their native hosts.
The reusable workflow caller passes empty repository secrets to Dell-routed PR
runs, and selected jobs also suppress credential inputs on that path. They retain
only the event's `GITHUB_TOKEN`, with `contents: read` and `actions: read`; no
permission increase or container-isolation change is introduced. Hosted/non-PR
secret handling stays unchanged. No new workflow files or required checks.

Both the default threshold mode and `MQ_RUNNER_MODE=prefer` use the same
bounded Dell queue admission. Prefer remains accepted for existing configurations;
unknown modes fall back to hosted.

Set the optional repository Actions variable `MQ_RUNNER_DEMAND` to tune the
per-run idle-capacity requirement without a workflow change. Unset or empty
uses the unchanged conservative default `6` (checked by `ci/test_ci_scope.py`).
In #4980's per-job routing, the Dell-routed heavy work in
`build-project.yaml` is one Tier 2 Linux build, two complete Tier 2 test jobs
(OFF/ON), two main Cross Lint legs, one release-target Cross Lint job, and the
Linux macOS Rust artifact producer (#5008). These are seven jobs in total, with
**a peak of six concurrent heavy jobs**: the two Tier 2 tests wait for their build
and can overlap all three Cross Lint jobs and the Darwin producer. The release
job runs its three targets within one job. The unchanged default demand `6`
covers that peak; an earlier override of `5` no longer covers it. A demand of `7`
is the conservative total-job bound. Before the Darwin producer, #4980's six-job
topology had a peak of five. These counts describe those topologies, not a fixed
admission constant.

**Revisit `MQ_RUNNER_DEMAND` whenever heavy jobs are added or their dependencies
change.** Inventory every job using `mq-heavy-labels` in
`.github/workflows/build-project.yaml`, expand its matrix, and account for
`needs` dependencies when estimating concurrent demand. A job count alone is
not the peak: Tier 2 build and tests run in successive stages. A demand above
the Dell's online capacity intentionally selects hosted. The selector uses the
configured demand and actual queued-job labels; it does not hard-code a five-job
peak or automatically infer future jobs before they appear in the API. The same
configured requirement applies when path scope skips some heavy jobs.
`MQ_RUNNER_MIN_IDLE` is an optional repository Actions variable (default `1`);
the idle fast case requires `max(MQ_RUNNER_MIN_IDLE, MQ_RUNNER_DEMAND)` runners
after subtracting queued Dell jobs. Online capacity must cover that requirement
even for busy admission, so offline or undersized pools still select hosted.

`MQ_RUNNER_MAX_RUNS` caps concurrent committed Dell merge-group runs, including
the new admission. Unset or empty defaults to **3**; nonpositive or noninteger
values select hosted. Below this cap, use the idle fast case when available,
otherwise queue on the busy Dell. At the cap, select hosted even if the runner
census is idle. This cap replaces `floor(online / required)`; busy runners and
queued jobs no longer force hosted fallback while a run slot remains.

The default comes from the six-runner Dell measurements on 2026-11-19 (reports
22/23, Refs #4896): 6 of 13 runs went hosted because 4–6 runners were busy with
required capacity 3. Dell completion had a 27.8-minute median against hosted's
46.3 minutes, while Dell job queue wait was p90 349 seconds and max 615 seconds.
Adding even that observed maximum wait to Dell's median gives 38.05 minutes,
8.25 minutes ahead of hosted's median. Three concurrent runs is a bounded first
step above the old two-run cap; tune the variable from subsequent completion
measurements rather than assuming this estimate is a measured overcommit result.
This queue policy does not authorize PRs to consume their reserved MQ capacity;
member-PR routing (#5279) retains its separate idle floor and queue reserve.
With PR routing enabled, retain capacity-weighted commitments and use a queue
budget of `MQ_RUNNER_MAX_RUNS * required` so a larger PR cannot count as one
smaller queue run. The cap does not relax PR admission, even with a zero reserve.

Runner IDs visible in both scopes count once (a busy observation wins). The job
summary records online/busy/idle counts, required capacity, queued Dell jobs,
committed runs, run limit, idle run slots and the idle/busy admission reason.
Queued/in-progress `ci.yml` merge-group runs and latest-attempt jobs establish
commitments:

- Live Dell-labelled jobs commit one run slot, including newer runs. Completed
  heavy jobs establish the route but release capacity when no Dell jobs remain,
  including runs waiting only for hosted macOS work.
- Older undecided runs reserve a slot, ordered by creation time then run ID.
  Identical snapshots therefore admit the same oldest runs, including runs
  created in the same second. A successful selector still has an unknown route
  until heavy-job labels appear: its output is not exposed by the jobs API.
  Keep its provisional reservation through that visibility gap (also when its
  conclusion is missing), preferring brief under-use over overcommit. Hosted
  choices, unsuccessful/skipped selectors with no live Dell jobs, and terminal
  runs release reservations. Inspect jobs even for workflows reporting `queued`.
- A stable retried run listing may omit the current run (run `44586086466`). Without
  its timestamp, count every undecided peer as older, and log the missing run ID
  rather than raising `KeyError`. Missing run timestamps sort before known peers;
  a missing current timestamp reserves every undecided peer. Missing run IDs
  hold a slot without a jobs lookup. Missing event/status fields use the filtered
  endpoint's values. Missing job fields similarly reserve that
  peer's run and required demand, with the missing key named in the warning.
  Run `44586086466` logged a bare `KeyError` after a snapshot retry; its API
  response was not retained, so tests reconstruct both missing-current and
  missing-field shapes rather than claiming an exact historical payload.
- Jobs-budget exhaustion reserves each unprobed run and at least its required
  demand (or its larger observed queued-job count). API errors, missing Actions
  identity/token and unresolved collections still select hosted.

Capacity-driven hosted routing is identified in the log and summary. This is a
bounded snapshot, not an atomic reservation: API visibility lag or runners going
offline after selection can still change capacity. Existing fixtures reproduce
the four-run burst, hosted backlog and collection races in `ci/fixtures/`.
Report 15 measured 168 idle Dell minutes (six idle runners, required three),
while 78 Dell-routable hosted jobs waited over five minutes (803 job-minutes),
and only 3 of 19 queue runs used the Dell. Read-only selector logs for
[44359171975](https://github.com/obs-rust/obs-studio/actions/runs/44359171975/job/134995483499)
and [44367435932](https://github.com/obs-rust/obs-studio/actions/runs/44367435932/job/135024874840)
show committed counts five and three despite six idle runners. Run
[44367448224](https://github.com/obs-rust/obs-studio/actions/runs/44367448224/job/135024915358)
instead fell back on a short queued-runs page with a mismatching count; bounded
snapshot retries now cover that race. The response-shape fixtures and peer job
timings in `ci/fixtures/mq-probe-races.json` reproduce #5181's collection failures
and the completed-selector visibility gap. Improved hosted timing and utilization still require
measurement after deployment.

The intended host is a NixOS netbooted Dell PowerEdge, which may be powered off.
Run the runner and its job processes inside its Ubuntu container image / FHS
environment with Ubuntu apt available: the existing jobs expect Ubuntu package
and toolchain prerequisites, not native NixOS commands. An offline PowerEdge
automatically falls back to hosted CI at selection time. This is a snapshot,
not a reservation: a host going offline after selection can still delay jobs.

The Tier 2 Linux build mounts `/var/cache/obs-runner` and selects a per-runner
persistent slot only when `.persistent-cache` exists and `RUNNER_NAME` is set.
On this Dell path, the build step sets `OBS_TIER2_LAUNCHER` to an empty value
to compile C/C++ without a launcher; hosted runners keep `zccache`. The
validation script uses `${OBS_TIER2_LAUNCHER-zccache}`: unset defaults to
zccache, a nonempty value selects another launcher, and empty omits both
launcher `-D` arguments and clears old launcher cache entries in a reused
CMake tree. zccache stats, daemon shutdown and recovery run only when zccache
is selected; the uncached path reports that stats were skipped.

This bypass follows the measured Dell daemon bottleneck (2.2 active compilers
with 65.8 waiting clients at 8.6 compiles/s; Refs #4954 and
[zccache#2209](https://github.com/forge-ci/zccache/issues/2209)). Dell A/B build
timing is pending while the host is offline for its hardware change. No
no-launcher speedup has been measured.

On the Linux x64 host chosen by the maintainer, open repository **Settings →
Actions → Runners → New self-hosted runner**, select Linux/x64, and follow the
shown download and verification commands. Use the temporary registration token
shown there to register the runner:

```sh
./config.sh --url https://github.com/obs-rust/obs-studio --token <temporary-token> --labels obs-mq
```

The runner adds the `self-hosted`, `linux`, and `x64` default labels. Install/start it as a service
using the runner's documented service commands, then set `MQ_RUNNER_LABELS`
only after it is online and the App credential is configured. Container
jobs require Docker installed and usable
by the runner account; the Tier 2 build and Flatpak jobs use `container:`, though
they are not selected on the trimmed queue path. This change neither registers
nor configures a runner. Clear the variable to return to hosted Linux runners.

### Kept Tier 2 configure reuse

Only the marker-backed kept-tree path sets `OBS_TIER2_KEEP_CONFIGURE=1`.
It disables the libobs module cache, including its restore/save steps: Dell
measurements restored 0/7 modules in five jobs, while enabling the imported
module list required a second configure. Hosted runners keep the existing
configure and module-cache behavior.

`ci/tier2_configure.py` stores `.tier2-configure.json` inside `tree/`, so the
existing crash-safe kept-tree save/restore carries it with `CMakeCache.txt`.
The SHA-256 input fingerprint covers the complete argument vector (preserving
argument boundaries and order): source/build paths, generator, every `-D`/`-U`,
compilers and launchers, build type, test/reference switches, extra flags and
version/build-number overrides. It also covers the working directory; PATH,
HOME, compiler/linker flags, locale, DISPLAY and SDK variables; every CMAKE_,
PKG_CONFIG, OBS_, SOLDR_, RUST, CARGO_, CCACHE_, ZCCACHE_, UV_ and PYTHON environment input
(including `OBS_TIER2_LAUNCHER` and the existing shell toolchain-version result); resolved
CMake/Ninja/compiler/launcher/soldr/pkg-config/git/uv/Python/linker/archiver
executable paths and contents;
the Ubuntu installed-package/version inventory; explicit toolchain
file contents; and Rust toolchain/Cargo configuration files.

OBS embeds `git describe --always --tags --match '[0-9]*.[0-9]*.[0-9]*'
--dirty=-modified` during configure. Its output is hashed too unless an explicit
`OBS_VERSION_OVERRIDE` replaces it. A different commit/tag/dirty version must
configure to avoid reporting a stale version: the first configure saving is
therefore limited to unchanged-version retries. Removing the module-cache
configure also benefits changed revisions. The measured 128–133 second Dell
warm configure is an opportunity, not a guaranteed per-commit saving.

Preload scripts (`-C`, including their arbitrary includes), presets and
`--fresh` conservatively retain explicit configure.
Reuse additionally requires matching contents of the cache, Ninja graph and
rules, the cached compiler/toolchain binaries/files, and an enabled Ninja CMake
regeneration rule. Missing/invalid stamps, trees, probes or any mismatch run
configure. The stamp is invalidated before configure and written atomically
only after success. Module-cache use never permits reuse. CMakeLists/includes
and `CONFIGURE_DEPENDS` changes remain Ninja's responsibility through
`cmake --build`; no build or test result is cached by this decision.

### Prebaked Dell Tier 2 packages

The independent master-push `tier2-image` job publishes the content-addressed
public GHCR package described in
[`rust/tools/linux-validate/tier2-image/README.md`](../rust/tools/linux-validate/tier2-image/README.md).
The first master push after merge seeds it; Dell selection switches to its
immutable digest once the public manifest exists. Hosted runners keep their
current image paths. Probe errors fall back, and the required gate does not wait
for publication. Baked package versions are frozen until an image input changes;
stock installs resolve current Ubuntu archive versions. Tests continue checking
the exact runtime package closure from their build artifact.

Prepared-runtime delta installs use APT's native `Acquire::Retries=3` with
15-second HTTP/HTTPS connection and data timeouts. Each update/install runs
under `timeout --kill-after=10s 180s`, allowing four times the observed roughly
45-second normal setup. An unavailable pinned version triggers one index
refresh and one exact-version install retry. An install that the bound stops
(exit 124 or 137, for example on a slow mirror) runs `dpkg --configure -a` and
is retried once; the downloaded `.deb` files stay in the archive mount (#5504).
An update timeout and other errors fail at once. Persistent failures report the
delta packages and fail the job.

The Tier 2 containers fetch from one mirror at a time and change mirrors
between attempts (#5510): first the runner's own (the first plain-HTTP,
non-security entry of the host's `/etc/apt/apt-mirrors.txt`, which the build
container mounts read-only at `/run/host-apt`), then `archive.ubuntu.com` and
`mirrors.edge.kernel.org`, over HTTP because the stock container has no CA
certificates before `Install Git`. A mirror that accepts connections and never
answers keeps `apt-get update` past its bound on every index file, and apt's
`mirror+file:` method never drops it, so a mirror list alone does not help.
`ci/host_apt_mirrors.py` holds the list and the rule; the build step carries
the same rule as shell, and `ci/test_tier2_apt_cache.py` checks both against
the same cases. The runtime delta's timeout retry fetches the index from the
next mirror. So does each retry of the build's `Install Tier 2 Packages`
(#5518): `Install Git` keeps its mirror order in `/etc/apt/ci-tier2-mirrors`,
and a retry moves from the mirror `Install Git` ended on to the next one, since
a mirror can answer the bootstrap and then serve the ~400 packages too slowly
for the install's 300 s bound. The security pocket keeps `security.ubuntu.com`.

The separate prepared release runtime is refreshed manually. Its producer is
`ci/tier2_setup_probe.py produce`, invoked by the existing `ci.yml`
`tier2-setup-producer` job; it is not the GHCR `tier2-image` job. Request a
refresh when the consumer logs show more than four delta packages (the fallback
limit remains sixteen). A maintainer performs these steps:

1. Dispatch the existing `ci.yml` on `master` with `tier2-setup-probe=true`:
   `gh workflow run ci.yml --repo obs-rust/obs-studio --ref master -f tier2-setup-probe=true`.
   This runs normal validation and the setup experiment; do not replace the
   current runtime until both succeed.
2. Before their one-day retention expires, download `tier2-setup-image` and
   `tier2-setup-metadata` from that run. The producer installs the new build's
   exact `runtime-packages.txt` plus bootstrap tools into the pinned stock base,
   commits the container, and saves the image. `bundle.json` records its image
   ID, complete inventory, base, archive checksum/size and producer revision.
3. Publish a **new, unique prerelease** via the existing `obs-studio-libs`
   pack/add catalogue process, as in
   [obs-studio-libs#7](https://github.com/obs-rust/obs-studio-libs/pull/7).
   Package the Docker-save contents with flavor `ubuntu26.04-docker` and platform
   `linux/x86_64`; this is an OCI archive, not a host library prefix. Never
   overwrite the October asset. If repacking changes the compressed bytes,
   compute the checksum and size of the final published archive.
4. Open a PR updating `rust/tools/linux-validate/prepared-runtime.json`: use the
   new immutable release URL, final archive checksum/size, `bundle.json` image
   ID/base/full inventory/revision/run ID and new catalogue version. Retain
   `platform: linux/amd64`. Run the runtime selection/delivery self-tests, then
   qualify both complete OFF/ON suites through ordinary PR/queue validation,
   checking `inventory_verified` and the remaining delta. Promote the release
   and catalogue only after qualification, following the original rollout.

The consumer cannot publish or dispatch a refresh. No repository variable
change is required. Hosted GHCR selection remains deferred while the package
is private; an anonymous manifest probe alone does not guarantee a subsequent
job-container pull will succeed.

Prepared release runtimes first inspect the descriptor's pinned Docker image ID
and reuse an exact match. Dell runners use `/var/cache/obs-runner/assets` only
when that directory and the `/var/cache/obs-runner/.persistent-cache` file both
exist. Environment variables cannot bypass this check. Hosted runners without
the persistent marker and directory retain cold delivery.

Archive filenames are their SHA256 (no extension). Reads copy into the job
directory, verify size and SHA256, then refresh the cache entry's access time for
host GC. A miss downloads directly into `.<sha256>.<random>.part`, verifies it,
sets mode `0644`, and atomically renames it into place. Existing entries remain
untouched for host GC, including corrupt files another uid may own. The job uses
a verified local copy. Any cache I/O error, including an access-time permission
error, falls back to downloading into the job directory. Downloaded bytes and
loaded image IDs retain their integrity checks. Image and asset cache hits/misses
appear in the setup log. The host owner reports all six Dell runners restarted
with the HDD-backed mount deployed on 2026-11-18.

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

### Windows compiler cache migration hold

The Windows app, Tier 2 and release packaging still use the Visual Studio
generator and ccache. zccache 1.15.17 supports MSVC's `/Fd` with embedded
debug information (`/Z7`). A compiler invocation that writes a shared PDB
(`/Zi` or `/ZI`) remains outside that cache path. The release CLI dispatches
by executable name only for its daemons, so copying zccache to `cl.exe`
cannot implement the existing MSBuild tool override. See the
[CLI entry point](https://github.com/forge-ci/zccache/blob/1.15.17/crates/zccache/src/bin/zccache.rs#L30-L59)
and [MSVC side-output classifier](https://github.com/forge-ci/zccache/blob/1.15.17/crates/zccache-compiler/src/side_outputs.rs#L19-L31).

A second blocker is explicit MSVC PCH handling. CMake adds `/Fp` when it
creates and consumes a PCH, and zccache's MSVC parser deliberately bypasses
compiles carrying that flag. The 1.15.22 source retains the same check. On
Linux, driving zccache 1.15.17 with a stand-in named `cl.exe` and
`/c /Z7 /Fdunit.pdb /Yuunit.h /Fpunit.pch` twice reported two
non-cacheable compilations. The stand-in verifies classification only.
See the [CMake MSVC PCH flags](https://github.com/Kitware/CMake/blob/v4.1.3/Modules/Platform/Windows-MSVC.cmake#L448-L449)
and the [zccache parser test](https://github.com/forge-ci/zccache/blob/1.15.22/crates/zccache-compiler/src/parse_msvc.rs#L740-L752).

A native executable shim needs qualification with the real MSBuild tool
task, including response files, embedded debug information, PCH creation
and reuse, and a warm build on x64 and ARM64. The Ninja alternative also
needs changes to the app's multi-architecture build setup.
`cmake/windows/architecture.cmake` reads Visual Studio platform metadata and
configures helper builds with the generator's `-A` option. Preserve the
current cache transport until a replacement demonstrates real hits and
the expected package contents. The
[Flatpak migration note](zccache-flatpak-migration-note.md) covers the
separate dependency on the shared CMake launcher and PCH helper.

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
record the useful numbers in the PR or issue comment before expiry; run
metadata is not tracked on `master` (#2686). Filesystem bytes
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

The [first native build-reuse attempt](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-22-native-cache-build-reuse.md)
failed qualification: Windows profiles were cold, and successful macOS app
builds lacked installed-binary freshness proof. Its retained evidence includes
the final failed gate and successful compatibility and protocol checks.

## Cross lint and Fast Gate overlap

The debug cross-lint matrix starts after scope and immutable-base admission,
alongside Fast Gate, as the release cross-lint lane already does (#4990).
It consumes no Fast Gate output or artifact; PR admission, cumulative queue
scope, targets, commands and CI Gate's required execution manifest are unchanged.
A failed or cancelled Fast Gate still blocks merging even if cross lint passes.

The November 17 sample of 30 completed `ci.yml` runs had 22 successful,
four failed, two skipped and two cancelled Fast Gates. A fixed-duration replay
removes 4.52–5.28 minutes from four of eleven successful hosted PR runs;
other heavy jobs mask the saving in the remaining seven PR runs and all
three successful queue runs. This is an overlap estimate, not a hosted
after-change measurement, and earlier runner allocation can change contention.
Cross lint now also spends runner time when Fast Gate fails: the observed
median combined debug-matrix execution was 11.87 hosted or 9.21 Dell
runner-minutes. All four failed Fast Gates used Dell runners, so full matrix
execution at that median would add about 36.83 runner-minutes across the
sample (1.23 per run), before cancellation savings. Successful runs add no work.

## Tier 2 execution and Fast Gate overlap

The Windows and macOS validators opt into `OBS_BUILD_NATIVE_TIER2` and build
the single `tier2-native` target. Its dependencies are the unchanged native
core/reference, cmocka registry and platform graphics/capture targets. Asking
the Visual Studio generator to build those roots separately starts MSBuild
once per root and revisits their shared dependencies (Refs #4996). The
aggregate preserves each existing compile/link recipe and test pass. Separate
configure/build markers make subsequent native job logs easier to measure;
macOS still redirects their detailed output to `build_rust.*.log`.

The complete OFF/ON test jobs start after successful Tier 2 compilation and
scope/proof checks, without waiting for Fast Gate. No artifact from Fast Gate
is consumed by these jobs. Fast Gate remains a required job in the reusable
workflow, and its failure/cancellation still makes the caller's required gate
fail even when both CTest suites pass. A missing/failed build or workflow
cancellation does not start tests. Partial Rust-only evidence conservatively retains Fast Gate and selected
Tier 2 work; complete client admission defers both to mandatory queue execution.

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
six concurrent tests. A host with more cores can raise that bound with
`OBS_GATE_TEST_THREADS` (a positive integer); CI sets nothing, so CI keeps
six. Native-build and pipe-parity tests start ahead of the bulk queue so their
slow tail overlaps ordinary tests (#4896). Native builds still run at most two
at a time; priority changes neither test selection nor concurrency, retries or
time budgets. The full profile inherits the same priority. The tool is installed
in a local cache only when scope
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
the snapshot `docs/ci-evidence/2026-10-22-nextest-log-observations.json`
(retired; at `c2af3f9c3`).
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
is unchanged: setup, compilation, debug processing, export, validators,
compiler reporting and cleanup all stay in its clock. Routine Flatpak builds
start cold (no Actions cache since #1801 phase A). Keep other probe inputs off for a
routine observation; optional Flatpak qualification probes remain separate
extra work when explicitly requested.

This mode saves measurement runner time and waiting on unrelated jobs. Do not
count those omissions as a Flatpak build speedup. Record full job/source/cache
identities, failures and costs; do not restart a live observation on API lag.
Manual master dispatches share their existing cancellation group, so wait for
the entire preceding manual master workflow to finish before starting another.

### Flatpak cache retention after migration (#1801)

The production build leaves Actions cache restore and save disabled.
Flatpak Builder's SDK ccache wrappers remain available for dependency
modules. The OBS module stages a checksum-pinned zccache launcher under the
source mount and uses a separate store inside the writable compiler cache
mount. Its Ninja wrapper closes the statistics session and stops the daemon
before writing the completed report. The explicit manual transfer probe
archives only durable state and checks the consumer's toolchain identity
before restoring. See the [migration decision](zccache-flatpak-migration-note.md).

The obsolete standalone compiler family no longer reserves 200 MiB. The
[saved policy replay](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-22-flatpak-legacy-retirement.md)
records the declaration reduction and unchanged deletion plans. This snapshot
has no standalone entries to reclaim; global overcommit and native architecture
retention remain open in #1801.

The [allocation and version-retention follow-up](https://github.com/obs-rust/obs-studio/blob/c2af3f9c3e7945566b88440dffb14c255aeebd96/docs/ci-evidence/2026-10-22-cache-allocation-followup.md)
maps all 25 remaining families, records overlapping publications and an actual
registry deletion across different host/container Actions versions, and links
the existing upstream identity defect. Its portable replay separates observed
deletions from hypothetical plans; global allocation remains unresolved.
