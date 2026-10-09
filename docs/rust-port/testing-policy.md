# Rust Port Testing Policy

**Status:** Mandatory. Every change that ports C/C++ code to Rust MUST follow
this policy exactly. If a rule cannot be followed for a specific unit, stop and
record the exception in the tracking issue under `## Decisions` before writing
code; do not silently deviate.

## Goal

OBS Studio keeps its **existing public API**, but its internals are rewritten
in Rust. Concretely, the following stay compatible:

- The **libobs C API and ABI**: every `EXPORT` function signature, every public
  struct layout, and every public header under `libobs/`. Existing plugins and
  the frontend must compile against the unchanged headers and work unmodified.
- The **obs-websocket v5 protocol**, byte-for-byte on the wire.
- The frontend API (`obs-frontend-api`), scripting bindings, and the
  `window.obsstudio` browser-source API.

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
- The **existing C header is the source of truth**. Do not generate a
  replacement header and do not edit the public header as part of a port.
  Public struct layouts MUST match the C layout exactly, including any
  awkward field types (e.g. `uint8_t pos` stays `u8` at the C boundary even if
  the safe core uses `usize`).
- A layout test MUST assert `size_of`, `align_of`, and every field offset of
  each `#[repr(C)]` struct against values measured from the real C header
  (via the oracle crate, see Tier 3).
- The CMake option `ENABLE_RUST_LIBOBS` (default `OFF` until the port is
  accepted) drops the ported C sources from libobs and links the Rust
  implementation in their place. Each port adds its C file to the
  `$<$<NOT:$<BOOL:${ENABLE_RUST_LIBOBS}>>:...>` guard in
  `libobs/CMakeLists.txt`. The unchanged C test MUST pass in **both**
  configurations.
- libobs MUST export exactly the same dynamic symbols with
  `ENABLE_RUST_LIBOBS=ON` as with `OFF`: no missing C symbols, no leaked Rust
  internals (`libobs/cmake/rust-exports.map` hides them on ELF).
- Run `rust/tools/linux-validate/run.sh` to check both requirements. It builds
  OFF and ON in Docker, runs the cmocka tests, and diffs the exported symbols.
  Paste its summary into the PR.
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
  `rust-exports.txt`; `export_list.rs` checks that every shim is exported or
  hidden.
- Windows: Rust `#[no_mangle]` symbols are not dllexport, so every C ABI shim
  symbol is listed in `libobs/cmake/rust-exports.txt`, which
  `libobs/cmake/rust.cmake` turns into `/EXPORT:` linker options.
  `rust/obs-util/tests/export_list.rs` fails if the list and the
  `#[unsafe(no_mangle)]` functions in `rust/*/src/ffi/` differ.
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
- The oracle for a unit is deleted only after the C source itself is deleted
  from the tree, in that same change.

### Tier 4 — Black-box protocol tests (external network API)

- Applies to obs-websocket and any other network-facing surface.
- Written against the running server over the wire, independent of the
  implementation language, and run unchanged against the C++ and the Rust
  servers.

## Required workflow for each ported unit

1. **RED:** commit the Tier 1 port of the existing test (and new edge cases)
   against stub implementations (`todo!()`), and show `soldr cargo test`
   failing.
2. **GREEN:** implement the safe Rust core until Tier 1 passes.
3. Add the C shim, the layout test, and the Tier 3 differential test.
4. Add the C file to the `ENABLE_RUST_LIBOBS` guard and run
   `rust/tools/linux-validate/run.sh`: the unchanged C tests pass `ON` and
   `OFF`, and the exported symbol sets are identical.
5. Record every intentional behavior difference in the tracking issue.

A port is not done until all applicable tiers pass. "Tier 1 passes" alone is
not done.

## Extracting a function before porting it

If the unit to port is one function inside a larger C file,
`ENABLE_RUST_LIBOBS` cannot drop it, because the guard drops whole files.

1. First move the function verbatim into its own `.c` file in the same
   directory.
2. Use a separate, behavior-neutral commit/PR that contains no Rust and changes
   no signature, header, or behavior.
3. Add the new file to `libobs/CMakeLists.txt`.
4. The unchanged C tests must pass.
5. Only then port the new file with the normal workflow, and add it to the
   `$<$<NOT:$<BOOL:${ENABLE_RUST_LIBOBS}>>:...>` guard.

Example: `os_get_path_extension` moves from `util/platform.c` to
`util/path-extension.c`.

## Characterization tests for units without C tests

If the C unit to port has no existing C test, Tier 2 has nothing to run. First
add a cmocka characterization test `test/cmocka/test_<unit>.c` that pins the
CURRENT C behavior, including odd or buggy behavior. Comment such cases as
"characterized, not endorsed".

- It lands in its own commit/PR BEFORE the port, contains no Rust, and passes
  with `ENABLE_RUST_LIBOBS` OFF (and ON, since nothing is swapped yet).
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
4. `profiler.c` and `source-profiler.c`
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
3. obs-websocket Tier 4 suite, then the port.

- Tier 3 parity is exact bit equality (NaN == NaN) where the C is scalar and
  deterministic. Only where the C uses SIMD (`vec3`, `vec4`, `matrix4` via
  SIMDe) is a per-function ULP tolerance allowed; document it in the parity
  test file.
- The bitstream parsers (`obs-avc.c`, `obs-hevc.c`, `obs-av1.c`) get
  cargo-fuzz differential targets against the C oracle. Each runs at least 1
  hour with no mismatch outside documented exclusions.
- Inputs that trigger C UB are excluded and listed. The `get_ue_golomb` UB is
  already fixed in C (#11, #17).
- obs-websocket gets a Tier 4 black-box suite covering every v5 request type,
  events, the auth handshake and error codes. It MUST pass against the current
  C++ server before any port.

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
  contract), `util/serializer.h` (`struct serializer`), and the `static inline`
  helpers in `util/dstr.h` (the `struct dstr` layout is the contract).
  `util/dstr.c` itself is ported with the normal workflow.

## Layout and naming

```text
Cargo.toml                    # workspace root, members = ["rust/*"]
rust-toolchain.toml           # pinned toolchain (required by soldr)
libobs/cmake/rust.cmake       # Corrosion import + whole-archive link into libobs
libobs/cmake/rust-exports.map # hides Rust internals from libobs exports (ELF)
libobs/cmake/rust-exports.txt # C ABI shim symbols to export (Windows /EXPORT:)
libobs/cmake/rust-unexports-macos.txt # hides Rust internals (macOS)
rust/
  libobs-rust/                # the ONLY staticlib; re-exports every port's ffi
  tools/linux-validate/       # Docker harness for Tier 2 (not a crate)
  tools/tier2-validate/       # macOS/Windows Tier 2 (validate.sh, validate.ps1)
  obs-util/                   # ports of libobs/util/*
    src/bitstream.rs          # safe core (Tier 1 target)
    src/{path_extension,darray,array_serializer,crc32,utf8,lexer,text_lookup,dstr,bmem}.rs # more safe cores
    src/ffi/bitstream.rs      # extern "C" shim, #[repr(C)] types (Tier 2)
    src/ffi/*.rs              # matching shims for the cores above
    tests/bitstream.rs        # Tier 1: 1:1 port of test/cmocka/test_bitstream.c
    tests/bitstream_layout.rs # Tier 2: struct layout vs. C header
    tests/bitstream_parity.rs # Tier 3: proptest vs. C oracle
  obs-graphics/               # ports of libobs/graphics/*
  obs-c-oracle/               # dev-only: original C compiled with oracle_ prefix
    build.rs
    oracle/bitstream.c        # #define renames + #include of libobs/util/bitstream.c
    oracle/{path_extension,array_serializer,darray,crc32,utf8,lexer,text_lookup,dstr,dstr_libc,bmem,test_stubs,platform_conv_host}.c
```

Port crates are plain `rlib`s. Only `libobs-rust` is a `staticlib`: each
staticlib embeds its own copy of `std`, so libobs must link exactly one. A new
port crate is added as a dependency of `libobs-rust` and re-exported there.

New libobs areas get their own crate under `rust/` named after the libobs
directory (`obs-util`, `obs-graphics`, `obs-media-io`, ...).

## Rules that apply everywhere

- Use `soldr cargo ...`, never bare `cargo`.
- `soldr cargo clippy --all-targets -- -D warnings` and `soldr cargo fmt --check`
  must be clean.
- `unsafe` is allowed only in `src/ffi/` shims, in `obs-c-oracle`, and in the
  Tier 2/3 test files that call C ABI functions. Each `unsafe` block has a
  `// SAFETY:` comment.
- Do not add new GitHub Actions workflow files without explicit sign-off from
  the maintainer. Hook tests into existing workflows instead.
- Do not edit a public C header, an existing C test, or an exported function
  signature as part of a port. Changing the public API is a separate,
  explicitly approved decision with its own issue.
