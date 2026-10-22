# Rust Port Testing Policy

**Status:** Mandatory. Every change that ports C/C++ code to Rust MUST follow
this policy exactly. If a rule cannot be followed for a specific unit, stop and
record the exception on the tracking issue under **Decisions:** (the format
[coordination.md](coordination.md) §2 uses) before writing code; do not
silently deviate.

## Goal

OBS Studio keeps its **existing public API**, but its internals are rewritten
in Rust. Concretely, the following stay compatible:

- The **libobs C API and ABI**: every `EXPORT` function signature, every public
  struct layout, and every public header under `libobs/`. Existing plugins and
  the frontend must compile against the unchanged headers and work unmodified.
- The **obs-websocket v5 protocol**, byte-for-byte on the wire.
- The frontend API (`obs-frontend-api`), scripting bindings, and the
  `window.obsstudio` browser-source API.

### Public and internal boundaries

Public headers, exported symbols, opaque-handle behavior, and public struct
layouts are compatibility contracts. `obs-internal.h` is a private header,
not a public ABI contract: its layouts may change when the C users of a field
have been ported or moved behind an internal accessor. Inventory those users
before changing a field. Adapt remaining C callers in a separate,
behavior-neutral commit or PR, with the existing C tests still passing;
then port the isolated unit. Do not edit a public header or an existing C test
to accommodate the Rust implementation.

This distinction matters for Phase 6. Core files share `obs-internal.h` by
direct field access, while `obs-data.h` exposes opaque handles whose layouts
live only in `obs-data.c`. The first core seam is `obs-data.c`, after its C
behavior has been characterized. Large files enter through the function
extraction workflow below: extract a C unit verbatim and land that change
before replacing the extracted file with Rust. The four tiers still apply to
each resulting unit, including symbol parity and differential tests.

Tests exist to prove two separate things: that the Rust code is correct, and
that it is a drop-in replacement for the C code it replaces. One kind of test
cannot prove both, so every ported unit carries the four tiers below.

## The four tiers

### Tier 1 — Rust unit tests (correctness of the Rust core)

- Written in Rust with `#[test]`, against the **safe Rust API** (no `unsafe`,
  no FFI types).
- Every assertion in the existing C test for that unit (if any) is ported 1:1
  into Rust, in a test whose name references the C test it mirrors.
- New edge cases are added here.
- Run with `soldr cargo test`.

### Tier 2 — Existing C tests against the Rust implementation (API/ABI compatibility)

- The existing C tests (`test/cmocka/*.c`, and any other C/C++ test for the
  unit) are **kept and never rewritten or edited** to accommodate the port.
  They are the compatibility check.
- The Rust crate exports the original C symbols through a thin
  `#[no_mangle] pub extern "C"` shim with `#[repr(C)]` structs. The shim only
  converts types and delegates to the safe core; it contains no logic.
- The **existing public C header is the source of truth** for the public ABI.
  Do not generate a replacement header or edit the public header as part of a
  port.
  Public struct layouts MUST match the C layout exactly, including any
  awkward field types (e.g. `uint8_t pos` stays `u8` at the C boundary even if
  the safe core uses `usize`).
- A layout test MUST assert `size_of`, `align_of`, and every field offset of
  each `#[repr(C)]` struct against values measured from the real C header
  (via the oracle crate, see Tier 3).
- The normal application builds, links and installs **hybrid `libobs`**: the
  completed Rust implementations plus unported C/C++. Replaced C sources are
  excluded from this target. Rust is required; a missing toolchain is an error.
- Compatibility validation explicitly enables `OBS_BUILD_C_REFERENCE=ON` and
  builds `libobs-c-reference` as well. It is excluded from the default build,
  never installed, and shares the unported object library with production.
  Both binaries retain identical public symbols, file names and ABI versions.
  The unchanged tests pass against **both**: mode `OFF` loads the C reference,
  mode `ON` loads production hybrid `libobs`. These mode names belong to the
  test harness; branding does not select the implementation.
- Each port moves its C file's `#include` into a ported unity group
  (`libobs/build/libobs.ported-*+.h.c`), selected by the canonical
  `libobs/build/source-sets.json`. The reference keeps compiling that group;
  production links the Rust replacement. Keep C reference/oracle sources during
  migration; their exclusion from production does not end parity coverage.
- Hybrid `libobs` MUST export exactly the C reference's dynamic symbols:
  no missing C symbols or leaked Rust internals. Keep platform export controls.
- Run `rust/tools/linux-validate/run.sh` to check both requirements. It builds
  once in Docker, runs the cmocka tests against each libobs, and diffs the
  exported symbols. PR CI's Tier 2 jobs run it and satisfy this requirement;
  paste its summary into the PR only if you ran it locally.
- Tests for the C boundary are written in C (they already are). Do not add
  C++ tests for C APIs.
- Memory that C code later frees or resizes (e.g. darray buffers) MUST be
  allocated with libobs `bmalloc`/`bfree`, declared once in
  `rust/obs-util/src/ffi/darray.rs`.
- `bmalloc`/`bfree` are now the Rust bmem port
  (`rust/obs-util/src/ffi/bmem.rs`, re-exported from `ffi/darray.rs`). Under
  `cargo test`, the base/platform symbols they and the oracles need come from
  the oracle crate (`oracle/test_stubs.c`, `oracle/file_serializer_host.c`,
  `oracle/platform_conv_host.c` and the real `util/base-variadic.c`), linked
  by every obs-util test via `use obs_c_oracle as _;`. Oracle copies of C
  that is now ported (e.g. `util/utf8.c`) are compiled under `oracle_*` names
  so they never clash with the Rust shims.
- C ABI shims for symbols libobs does not export (base.c internals, utf8) are
  listed in `libobs/cmake/rust-hidden.txt` and hidden via `local:` in
  `rust-exports.map` and `rust-unexports-macos.txt` instead of being listed in
  `rust-exports.d/`; `export_list.rs` checks that every shim is exported or
  hidden.
- Windows: Rust `#[no_mangle]` symbols are not dllexport, so every C ABI shim
  symbol is listed in `libobs/cmake/rust-exports.d/<crate>-<ffi module>.txt`
  (one fragment per FFI module, so ports don't edit a shared list), which
  `libobs/cmake/rust.cmake` turns into `/EXPORT:` linker options.
  `rust/obs-util/tests/platform/export_list.rs` fails if a fragment and the
  `#[unsafe(no_mangle)]` functions in its `rust/<crate>/src/ffi/<module>.rs`
  differ.
- macOS: `libobs/cmake/rust-unexports-macos.txt` hides Rust internals with
  `-unexported_symbols_list`.
- CI: `.github/workflows/build-project.yaml` runs the Rust tests on Linux,
  macOS and Windows (job `rust-tests`). It runs
  `rust/tools/linux-validate/run.sh` (job `rust-linux-tier2`) and
  `rust/tools/tier2-validate/validate.sh` / `validate.ps1` on macOS and
  Windows (job `rust-tier2`). Each OFF/ON pair must export identical symbol
  sets.

### Tier 3 — Differential tests against the original C (behavioral parity)

- The original C source is compiled into the Rust test build as an **oracle**
  through the `rust/obs-c-oracle` crate (a dev-dependency only, built with
  the `cc` crate). Oracle symbols are renamed with an `oracle_` prefix by a
  wrapper `.c` file that `#define`s each exported name and then `#include`s
  the original source file. The original file is not copied or modified.
- When the C ABI forces a behavior the safe API should not have (e.g. a
  `uint8_t` position that wraps), make the core generic over it rather than
  duplicating logic in the shim; see `Position` in
  `rust/obs-util/src/bitstream.rs`.
- A `proptest` test feeds random inputs to both the oracle and the Rust shim
  and asserts identical outputs and identical observable state.
- A mutation check is expected once per port: temporarily break the core and
  confirm the differential test fails, and say so in the PR.
- Known, intentional differences from the C behavior (bug fixes) MUST be
  listed in the test file and in the tracking issue, and the property test
  must exclude exactly those inputs, no more.
- When a C source is removed from the production tree, move that original
  source to `rust/obs-c-oracle/legacy/` (created by the first such removal;
  it is the one allowed copy of ported C) in the same change and update its
  oracle wrapper. Keep Tier 3 parity running; the production source's
  deletion does not end the behavioral baseline. See
  [release-criteria.md](release-criteria.md) for the release and rollback
  gates before removing a C implementation.

### Tier 4 — Black-box protocol tests (external network API)

- Applies to obs-websocket and any other network-facing surface.
- Written against the running server over the wire, independent of the
  implementation language. The suite MUST pass against the unchanged C++
  obs-websocket plugin loaded by both libobs builds (the C reference and
  hybrid). The plugin is never ported (AGENTS.md).

## Required workflow for each ported unit

1. **RED:** commit the Tier 1 port of the existing test (and new edge cases)
   against stub implementations (`todo!()`), and show `soldr cargo test`
   failing.
2. **GREEN:** implement the safe Rust core until Tier 1 passes.
3. Add the C shim, the layout test, and the Tier 3 differential test.
4. Move the C file's `#include` into a ported group (`libobs/build/libobs.ported-*+.h.c`,
   [BUILD.md](../../BUILD.md)) and run `rust/tools/linux-validate/run.sh`: the unchanged C tests pass against
   `libobs-c-reference` and hybrid `libobs`, and the exported symbol sets are identical.
5. Record every intentional behavior difference in the tracking issue.

A port is not done until all applicable tiers pass. "Tier 1 passes" alone is
not done.

PR CI on `obs-rust/obs-studio` decides whether the tiers pass for a merge.
Open a PR (a draft is fine) once the RED commit exists and let CI run the
tiers; running them locally first (`./lint`, `./unittest`, `local-gate run`)
is recommended, not required. The tiers, RED then GREEN, and the ban on
editing C tests or public headers stay mandatory.

## Extracting a function before porting it

If the unit to port is one function inside a larger C file,
Hybrid `libobs` cannot drop it, because the ported lists hold whole files.

1. First move the function verbatim into its own `.c` file in the same
   directory.
2. Use a separate, behavior-neutral commit/PR that contains no Rust and changes
   no signature, header, or behavior.
3. Add the new file to a libobs unity group (`libobs/build/`, [BUILD.md](../../BUILD.md)).
4. The unchanged C tests must pass.
5. Only then port the new file with the normal workflow, and move its
   `#include` into a ported group.

Example: `os_get_path_extension` moves from `util/platform.c` to
`util/path-extension.c`.

## Characterization tests for units without C tests

If the C unit to port has no existing C test, Tier 2 has nothing to run. First
add a cmocka characterization test `test/cmocka/test_<unit>.c` that pins the
CURRENT C behavior, including odd or buggy behavior. Comment such cases as
"characterized, not endorsed".

- It lands in its own commit/PR BEFORE the port, contains no Rust, and passes
  against the C reference and hybrid `libobs` (nothing is swapped yet).
- From then on it is an existing C test under Tier 2: the port may not edit
  it. Tier 1 ports its assertions 1:1 into Rust.
- Register it in `test/cmocka/CMakeLists.txt` and add it to the `cmocka-tests`
  aggregate target like the others.
- If the C source is not exported from libobs (e.g. `util/utf8.c`), compile
  that C source into the test executable, as `test_formatted_filename` does.
  The test must then be marked as characterizing the C source only: it cannot
  exercise the Rust swap until the symbol is reachable.
- Use only portable behavior, or guard platform-specific cases with
  `#ifdef _WIN32` / `#ifndef _WIN32`.

### Port order for Phase 3 (pure leaf utilities)

1. `crc32`
2. `utf8`
3. `lexer.c` / `cf-lexer.c` / `cf-parser.c`
4. `text-lookup.c`
5. `dstr.c`
6. `base.c` (done: the variadic `blog`/`blogva`/`bcrash` stay in
   `util/base-variadic.c`). `cf-lexer.c` / `cf-parser.c` from step 3 are
   deferred to a follow-up.
7. `bmem.c` last: a wrong allocator breaks every other Tier 2 run.

- The `utf8` and `dstr` Tier 3 parity tests MUST generate arbitrary byte
  strings, including invalid UTF-8.
- The `bmem` port MUST match the 32-byte alignment, the
  `bmalloc`/`brealloc`/`bfree` hooks, and `bnum_allocs` accounting:
  `brealloc(NULL, n)` counts as an allocation, and `bfree(NULL)` does not
  decrement.

### Port order and rules for Phase 4 (stateful, I/O and platform utilities)

The characterization tests `test_file_serializer`, `test_config_file`,
`test_threading`, `test_task`, `test_pipe`, `test_profiler` and
`test_platform` land first (the first Phase 4 PR) and are thereafter unchanged
Tier 2 checks.

1. `file-serializer.c` and `buffered-file-serializer.c`
2. `config-file.c`
3. `task.c`
4. `profiler.c` (`source-profiler.c` is Phase 6 core work because it accesses
   `obs-internal.h` and source/graphics state directly)
5. `pipe.c` / `pipe-posix.c` / `pipe-windows.c`
6. `threading-posix.c` / `threading-windows.c`
7. `platform.c`, then `platform-nix*.c` / `platform-windows.c`

- Every test that touches files uses its own temp directory, relative to the
  ctest working directory, and removes it in teardown.
- No sleeps: synchronize on events, semaphores or joins.
- Threading tests MUST be deterministic: 100 consecutive runs without a flake.
  Check it with the Tier 2 harness, which reruns the given tests in both the
  OFF and ON builds after the normal run, stopping at the first failure
  (`ctest --repeat until-fail:N`):
  `rust/tools/linux-validate/run.sh --repeat 100 --tests 'test_threading|test_task'`
  (`tier2-validate/validate.sh` takes the same options on macOS;
  `validate.ps1 -Repeat 100 -Tests '...'` on Windows).
- Tier 3 parity for I/O code runs the oracle and the Rust shim against
  separate temp directories and compares the resulting files byte for byte,
  plus the return values.
- The `config-file` parity test MUST cover arbitrary INI input, including
  malformed lines.
- The threading and task ports add a loom or stress-style Rust test on top of
  parity.
- Per-OS files are ported per OS. A port is done only when that OS's Tier 2
  passes in CI.

### Port order and rules for Phase 5 (beyond util)

1. Graphics math in crate `obs-graphics`: `vec2.c`, then `vec3.c`, `vec4.c`,
   `quat.c`, `matrix3.c`, `matrix4.c`, `plane.c`, `bounds.c`, `axisang.c`,
   `math-extra.c`.
2. `media-io` in crate `obs-media-io`.
3. obs-websocket Tier 4 suite against the unchanged C++ plugin. Preserve it
   as the compatibility check required by `AGENTS.md`; the earlier
   [port proposal](websocket-port.md) is superseded.

- Tier 3 parity is exact bit equality (NaN == NaN) where the C is scalar and
  deterministic. Only where the C uses SIMD (`vec3`, `vec4`, `matrix4` via
  SIMDe) is a per-function ULP tolerance allowed; document it in the parity
  test file.
- The bitstream parsers (`obs-avc.c`, `obs-hevc.c`, `obs-av1.c`) get
  cargo-fuzz differential targets against the C oracle. Each runs at least 1
  hour with no mismatch outside documented exclusions.
- These and the later targets (`nal_diff`, `config_ini_diff`) run nightly
  through the local runner described in [fuzzing.md](fuzzing.md).
- Inputs that trigger C UB are excluded and listed. The `get_ue_golomb` UB is
  already fixed in C (#11, #17).
- obs-websocket gets a Tier 4 black-box suite covering every v5 request type,
  events, the auth handshake and error codes. It MUST pass against the
  unchanged C++ plugin on both libobs builds; the plugin is never ported.

## Header-inline code (static inline functions and macros)

Code that lives entirely in a public header as `static inline` functions or
macros is compiled into every caller, so no symbol swap is possible.

- Exemption: Tier 2 for such a unit is (a) a layout test of every public struct
  it defines (`size_of`, `align_of`, every field offset against the oracle) and
  (b) the unchanged C tests passing OFF and ON.
- Do not modify the header.
- Tier 1 and Tier 3 still apply. The Rust equivalent, which Rust ports use when
  they manipulate those structs, gets a 1:1 port of the C test plus a
  differential test. That test runs against oracle wrappers that instantiate
  the inline functions as non-inline `oracle_*` functions.
- This currently covers `util/darray.h` (the `struct darray` layout is the
  contract), `util/deque.h` (`struct deque`), `util/serializer.h`
  (`struct serializer`), and the `static inline` helpers in `util/dstr.h`
  (the `struct dstr` layout is the contract).
  `util/dstr.c` itself is ported with the normal workflow.

## Python runs through uv

Every Python invocation in this repo goes through `uv run` (or `uvx` for a
tool pulled from a package), never bare `python` or `python3`: `uv run
python -m unittest ci/test_x.py`, `uv run tools/leaderboard/generate.py`,
`uv run --with pillow python docs/images/generate-logo-assets.py SOURCE`. This
covers shell scripts, `./lint`, `./unittest`, `local-gate.toml`, workflows and
composite actions (which set uv up with the `astral-sh/setup-uv` step already
pinned in `build-project.yaml`), CMake `COMMAND`s, docstring usage lines and
Markdown code. Standalone scripts start with `#!/usr/bin/env -S uv run
--script` (with PEP 723 inline metadata when they need packages); a
shebang naming a bare interpreter is not allowed.

`ci/test_uv_run.py` scans every tracked file and fails on a bare invocation.
Its small, commented allowlist covers only trees we do not own (`deps/`,
`plugins/`). Do not extend it to dodge the rule.

## Layout and naming

```text
Cargo.toml                    # workspace root, members = ["rust/*"]
rust-toolchain.toml           # pinned toolchain (required by soldr)
libobs/cmake/rust.cmake       # Corrosion import + whole-archive link into libobs
libobs/cmake/rust-exports.map # hides Rust internals from libobs exports (ELF)
libobs/cmake/rust-exports.d/  # C ABI shim symbols to export (Windows /EXPORT:), one file per FFI module
libobs/cmake/rust-unexports-macos.txt # hides Rust internals (macOS)
rust/
  libobs-rust/                # the ONLY staticlib; re-exports every port's ffi
  tools/linux-validate/       # Docker harness for Tier 2 (not a crate)
  tools/tier2-validate/       # macOS/Windows Tier 2 (validate.sh, validate.ps1)
  obs-util/                   # ports of libobs/util/*
    src/bitstream.rs          # safe core (Tier 1 target)
    src/{path_extension,darray,array_serializer,crc32,utf8,lexer,text_lookup,dstr,bmem,cf_tokenizer}.rs # more safe cores
    src/ffi/bitstream.rs      # extern "C" shim, #[repr(C)] types (Tier 2)
    src/ffi/*.rs              # matching shims for the cores above
    src/platform/             # the only host-specific code (cfg_select!, unix/, windows/)
    tests/suite.rs            # the one obs-util test binary (Utilities category; see Test binary budget)
    tests/containers/main.rs  # a module of suite.rs, like config/, memory/, threading/, ...
    tests/containers/bitstream.rs        # Tier 1: 1:1 port of test/cmocka/test_bitstream.c
    tests/containers/bitstream_layout.rs # Tier 2: struct layout vs. C header
    tests/containers/bitstream_parity.rs # Tier 3: proptest vs. C oracle
  obs-graphics/               # ports of libobs/graphics/*
  obs-media-io/               # ports of libobs/media-io/*
  ../dylints/                 # repo Dylints run by ./lint (not workspace members)
  obs-c-oracle/               # dev-only: original C compiled with oracle_ prefix
    build.rs
    oracle/bitstream.c        # #define renames + #include of libobs/util/bitstream.c
    oracle/{path_extension,array_serializer,darray,crc32,utf8,lexer,text_lookup,dstr,dstr_libc,bmem,test_stubs,platform_conv_host,cf_tokenizer}.c
```

Port crates are plain `rlib`s. Only `libobs-rust` is a `staticlib`: each
staticlib embeds its own copy of `std`, so libobs must link exactly one. A new
port crate is added as a dependency of `libobs-rust` and re-exported there.

New libobs areas get their own crate under `rust/` named after the libobs
directory (`obs-util`, `obs-graphics`, `obs-media-io`, ...).

## Test binary budget

Minimize the number of Rust test executables that must be compiled and
linked. Use **one amalgamated test binary per category**. The inventory below
selects eight categories for the current workspace, including unit-test
harness coverage and integration tests. Eight is the result of this grouping,
not an allowance per crate. Prefer fewer when categories can share a harness
without losing meaningful separation.

The objective is fewer test compilation and link targets when building the
whole suite. The category binaries replace existing per-crate unit-test
harnesses and integration-test executables; they must not be added alongside
them. Moving only integration tests while leaving the same library unit-test
harnesses enabled does not complete consolidation. Production libraries
remain dependencies of the shared harnesses and retain their normal builds.
Do not compile an entire production source tree again through `include!` or `#[path]`
just to put its tests into an amalgamated binary: that would duplicate the
compilation we are trying to remove. The native build resolver is a narrow
exception: its small private helper and test share one source module with
the application harness, avoiding both a public test API and a second
compilation of the entire build engine.

| Category / selected binary | Existing suites and coverage to include |
| --- | --- |
| Utilities / `obs-util` library harness | All eight `obs-util` suites: config, containers, memory, platform, profiler, serializer, strings, threading; its inline platform tests; `obs-c-oracle/tests/oracle` oracle validation. |
| Core state / `obs-tests-core` | `obs-data/tests/data`, `obs-properties/tests/properties`, `obs-hotkey/tests/hotkey`, `obs-callback/tests/callback`, `obs-missing-files/tests/missing_files`, `obs-output/tests/output`, `obs-scene/tests/scene`, `obs-audio-monitoring/tests/audio_monitoring`, `obs-audio-controls/tests/audio_controls`, and `obs-core/tests/obs_core`. |
| Graphics / `obs-tests-graphics` | `obs-graphics/tests/linear`, `geometry`, and `shader`. |
| Media I/O / `obs-media-io` library harness | `obs-media-io/tests/audio`, `video`, and inline `video_output` tests. |
| Codecs and encoders / `obs-tests-codec` | `obs-codec/tests/codec` and `obs-nvenc/tests/nvenc`, including the NVENC plugin boundary coverage. |
| GPU status / `obs-tests-gpu` | `obs-gpu-status/tests/gpu_status`, including frontend adapter boundary coverage. |
| Preflight / `obs-tests-preflight` | `obs-preflight` CLI, FFI, and per-platform inline tests; `obs-preflight-ffi` boundary coverage. |
| Application / `obs-tests-app` | `obs-studio-rust/tests/cli`, including its CLI, JSON, and WebSocket client checks; `obs-build` native compilation contracts and private resolver check; feature-gated native libobs and `obs-sys` bundled contracts. Preserve the C++ OBS WebSocket plugin as required by AGENTS.md. |

Utilities and media use their library unit harness as the one category
executable. Their existing integration suites are modules in that harness,
so private unit tests retain access without extra binaries or exposed
internals. The other six categories use explicit integration targets.
`libobs-rust` remains the production aggregation library; its constituent
behavior belongs to the categories above. `obs-c-oracle` remains a shared
test dependency, not a separate executable. FFI adapters with no tests have
no empty harness. Fuzz targets, benchmarks, documentation tests, and unchanged
C compatibility executables retain their separate execution models.

Thread-safe cases run with libtest's normal parallelism. Serialize tests
sharing the profiler with their existing common lock. Tests that change
process-wide allocation counters, logging callbacks, or the working directory
must re-exec the same category executable with an exact test selector.
Pipe-spawning oracle checks also re-exec first: their C pipe-to-CLOEXEC
window must not overlap unrelated child spawns in this process.
File-replacement parity checks re-exec too: a concurrent Windows child can
inherit an open C stdio handle and prevent replacement after C closes it.
Crash probes also re-exec before forking, so they cannot inherit live pipe
handles from other tests. Do not serialize the whole suite or add another
compiled binary to obtain a clean process. Use `module_path!()` for exact
selectors so module regrouping cannot silently run zero child tests.
The application category retains `libobs-integration` and `bundled-native`
feature gates for expensive native contracts; these add cases to the same
executable, not new executables.

The preflight CLI parser lives in the safe engine's `cli` module, consumed
by its real executable and tested in the shared preflight suite. Its ABI
layout structs live in `ffi/types.rs`, consumed by the static C adapter and
the layout tests. Neither production source needs to be included a second
time in a test executable to retain those checks.

- Use the category map above to choose the shared test binary. A new test
  file, test tier, feature, or crate does not by itself justify another
  executable. Put cases in modules included by the component's harness.
- Keep every assertion, differential case, platform check, and required
  test tier. This policy reduces executable count, not test-case count.
- Avoid empty or duplicate library/binary test harnesses. Disable them with
  `test = false` only after their coverage is included in a shared harness.
- Count the harnesses Cargo actually builds for each supported platform,
  including automatically discovered targets. Report documentation tests
  and unchanged C compatibility executables separately; this Rust budget
  does not remove either kind of coverage.
- Any necessary split must identify a distinct component or technical
  isolation requirement and document why the existing categories cannot
  accommodate it. Do not split binaries just because tests live in different
  source files or crates.

`ci/test_binary_cap.py` discovers unit harnesses and integration targets from
manifests without requiring a Rust toolchain or compiling anything, including
implicit workspace path dependencies. It checks category ownership,
rejects duplicate and missing categories, and rejects any
extra unit or integration harness. Cargo metadata provides an independent
inventory check enforced by the lint lane on build hosts. The guard runs
through existing CI Python test discovery; `uv run ci/test_binary_cap.py --report` prints the inventory.

Use `soldr cargo test --workspace --locked` for the full suite. For focused
work, select the owning category, for example
`soldr cargo test -p obs-data --test obs-tests-core properties::` or
`soldr cargo test -p obs-util --lib tests::strings::`. A crate whose tests
belong to another category no longer has a separate executable. The fast
gate follows dev-dependencies too, so a change to that crate still selects
the category owner; `ci/test_rust_scope.py` guards these relationships.

Verify consolidation with Cargo's actual test target inventory and test
discovery before and after: all existing cases must still be discovered on
their supported platforms, and the old unit/integration targets must no
longer be built. Measure full-suite compilation and linking separately from
test execution. A smaller target count is the structural requirement; report
timing evidence before claiming a speed improvement.

## Rules that apply everywhere

- Use `soldr cargo ...`, never bare `cargo`.
- The Rust toolchain is pinned in `rust-toolchain.toml` to the latest version
  natively supported by the soldr version CI uses (`version:` of the
  `setup-soldr` step in `.github/workflows/build-project.yaml`). Check what a
  soldr release supports in its own `rust-toolchain.toml` (zackees/soldr, at
  that tag). Never pin beyond it; bump the toolchain only together with, and
  after, a soldr bump that supports it. The Tier 2 Docker image installs the
  same pinned version (`rust/tools/linux-validate/run.sh`). `./lint` enforces
  this: `ci/test_toolchain.py` holds the expected version
  (`SOLDR_SUPPORTED_RUST`) and fails if `rust-toolchain.toml`, the MSRV, or
  any Dockerfile, workflow or doc pin differs.
- `soldr cargo clippy --all-targets -- -D warnings` and `soldr cargo fmt --check`
  must be clean.
- `unsafe` is allowed only in `src/ffi/` shims, in `obs-c-oracle`, and in the
  Tier 2/3 test files that call C ABI functions. Each `unsafe` block has a
  `// SAFETY:` comment. Every port crate denies the `unsafe_code` lint, so a
  safe core cannot use it: `allow` is written only where the C boundary is
  (the `ffi` module) and on the platform modules that must call the OS
  directly (`obs_util::platform::windows`, `obs-media-io`'s `platform::unix`,
  each saying why). Crates with no C boundary (`libobs-rust`,
  `obs-studio-rust`) `forbid` it outright. Keep it that way when adding a
  crate.
- **Platform boundary** (#323). Host-specific code lives only in the crate's
  `src/platform/` module and in `src/ffi/` (the exported symbol set itself
  differs per OS). `src/platform/mod.rs` is the crate's single host-selection
  site: one `cfg_select!` that declares the per-OS children
  (`platform/unix.rs`, `platform/windows/`, ...) and re-exports a neutral API
  (types, functions, `const` flags, or a capability macro such as
  `with_audio_thread_port!` for items that exist only on some hosts).
  Everywhere else in `rust/<crate>/src/`, host `#[cfg]`/`#[cfg_attr]`/`cfg!()`
  (`windows`, `unix`, `target_os`, `target_env`, `target_arch`, ...) and native
  paths (`std::os::{unix,windows,..}`, `libc::`, `windows_sys`) are errors.
  `dylints/ban_platform_cfg_outside_boundary` enforces this in `./lint`
  (`soldr cargo dylint --all -- --workspace --lib --bins`). It reads the
  source before cfg expansion, so code compiled out on Linux is checked too;
  that is what lets one Linux lint lane stand in for every OS. Out of scope:
  `obs-c-oracle`, `rust/tools`, and `tests/`. There is no allowlist: move the
  code into `platform/`, do not opt out.
- Do not add new GitHub Actions workflow files without explicit sign-off from
  the maintainer. Hook tests into existing workflows instead.
- Do not edit a public C header, an existing C test, or an exported function
  signature as part of a port. Changing the public API is a separate,
  explicitly approved decision with its own issue.
