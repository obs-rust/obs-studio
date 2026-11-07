# Batch 5: real OpenGL visibility fixture

Refs #1390. Branch-only slice based on `origin/test/1390-b4-rollup` (#3877).

Register `test_visibility_transition` (eight unchanged C cases) in the shared
graphics graph. Preserve Linux plus `OBS::libobs-opengl` guards, libobs/cmocka
linkage, `frontend/utility/visibility-transition.c.h`, the frontend utility
include directory, renderer artifact definition and runtime working directory.
The original timeout is unset. No C test, public header, WebSocket, workflow
or Rust executable changes. No absolute inventory totals are added.

## Fixture investigation

Original CMake passes `$<TARGET_FILE:OBS::libobs-opengl>` as
`GRAPHICS_MODULE_PATH`, adds the renderer build dependency and runs in
`${OBS_OUTPUT_DIR}/$<CONFIG>` for libobs effect data. The Linux Tier 2 harness
runs CTest under Xvfb; these fragments do not individually start Xvfb or request
EGL surfaceless. The native graph already supports artifact definitions and
`${runtime}`; `build_graphics_tests` supplies the real built renderer, and the
existing native graphics harness stages data and runs both core variants.
No new fixture design or stub is needed.

The focused opt-in replay imports real libobs and cmocka through pkg-config,
finds the matching real OpenGL module in libobs's libdir, stages libobs data,
and exercises both Cargo/cc::Build and CMake/CTest. Run it under Xvfb (or an
existing real display). It forces native cmocka stdout and rejects skipped
cases on either stream. It also checks CMake Linux/renderer exclusions,
commands and timeout inventory. The registration contract covers Windows and
macOS exclusions without discovering a Linux SDK on those hosts.

## Deferred candidates

The original inventory has 30 graphics-fixture executables; this slice adds
one. The other 29 remain in their original CMake registrations.

- `test_raw_output_video_reset`: real OpenGL initializes, but installed OBS
  32.1.2 lacks reset behavior and `core_video_release`; four cases fail.
- `test_transition_audio_clock`, `test_transition_audio_rescale`: installed
  OBS 32.1.2 rejects current source/encoder structure sizes; cases fail.
- `test_transition_removed_child_audio`: CTest reports Passed but cmocka skips
  its case after the same source ABI mismatch. The strict replay detects it.
- The other 25 inventory entries were not changed; they require additional
  owning plugin, SDK, device or runtime investigation, or a later slice.

These are installed-library limitations, not evidence of final-head failures.
The first review caught the hidden cmocka skip and requested both-stream skip
checks; both findings were addressed by narrowing to visibility and enforcing
strict detection. One primary reviewer ran; no additional review agent.

## Evidence

- RED `cb51acd2f`: registration check fails on missing OpenGL candidates
  (`local-test-qActYg.log`). The initial five-test investigation was narrowed
  to the visibility fixture after the installed-library failures and hidden skip.
- Final visibility-only GCC replay: both registration and runtime contracts
  pass (`local-test-9CwEz6.log`). All eight unchanged cases pass through Cargo
  and again through CMake/CTest, with no skips.
- Clang replay of the same final Application harness passes both contracts and
  all eight cases through each consumer (`local-test-HT2E2f.log`).
- CMake runtime contract checks the enabled inventory and zero tests with the
  Linux or renderer fact disabled, direct command and original unset timeout.
- Focused graph sorting, portability, slice, cmocka compatibility and binary
  cap guards pass (`local-test-0lY1en.log`); no absolute totals changed.
- Registration cost, 25 reduced-debug runs: longest 0.087 seconds, peak child
  RSS 14,676 KiB; no own Rust threads. The native test creates OBS worker
  threads; the real compilation/CMake replay uses the existing build-budget
  exception. Cost artifact: `build_ci_audit/b5-gl/test-cost.json`.
- Full `SOLDR_PORCELAIN=0 SOLDR_RUSTC_WRAPPER=none ./lint` ran twice.
  The first run found formatting, corrected with scoped rustfmt. The second
  completed formatting and CI self-tests, then found a needless borrow in the
  new replay helper (`local-test-BdgyRP.log`). Removed that borrow; scoped
  all-target Clippy for obs-build/obs-studio-rust with libobs-integration and
  `-D warnings` passes (`local-test-BkKsBE.log`). No remaining reported source
  lint errors. The full post-fix feature matrix and Dylint are unverified.
  Diff check passes. One workspace formatting call was refused by clud's
  verification-ratio guard; no override was used, and formatting was scoped.

Logs are retained in `/home/niteris/.clud/tmp/tool-output/local-test-private/`.
This real installed OBS 32.1.2/llvmpipe/Xvfb smoke evidence does not establish
final-head hybrid versus C-reference or dynamic export parity. Application
profiles, actual device behavior and Windows/macOS runtime are unverified.
No target pruning, soldr gc, toolchain workaround or hosted CI load.
