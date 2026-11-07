# Batch 2: Linux Rust-static audio-monitoring tests (#1390)

This continuation branches from `test/1390-b2-rust-static2` and retains all
prior registrations. It adds six unchanged archive-only C executables:

- `test_audio_monitor_delay_steps_rust`
- `test_coreaudio_device_rules_rust`
- `test_coreaudio_pacing_steps_rust`
- `test_monitor_speaker_layout_rust`
- `test_monitor_video_sync_rust`
- `test_monitor_wasapi_steps_rust`

Each retains its original C source, include path, archive-first/cmocka-second
linkage, direct command and absent timeout, wrapper and skip code. Both CoreAudio
cases retain their original Rust-mode definitions and trailing `m` linkage.
Target and test require both Linux and the production `libobs_rust-static`
archive; macOS/Windows remain excluded as before. The original
`address,undefined` sanitizer declarations are preserved. No device, display,
graphics context, substitute core or replacement shim is needed. Existing C
tests, public headers, production implementations and WebSocket remain unchanged.

The registration and real runtime checks live in a separate module in the
existing Application category binary. Registration checks archive presence on
all three OS selections. The runtime contract builds and runs all six tests
through both the actual Cargo graph and CMake consumer, importing the real
production Rust archive. Its CMake fixture extracts the production
`obs_test_sanitize` function verbatim instead of substituting a no-op. CTest
inventory is checked with the archive disabled and enabled, including absent
TIMEOUT properties. No new Rust test executable or CI workflow is added.

RED: `67b78f7a5`, focused registration fails with `missing Rust-static monitor
steps` (exit 101). GREEN: all registrations and unchanged runtime cases pass.

Validation (Linux, archive-only evidence):

- GCC 15.2.0 and Clang 21.1.8 pass both Cargo and CMake/CTest runtime contracts.
- Scoped Application-test Clippy with `libobs-integration` passes, preserving
  the existing cognitive-complexity limit of 25.
- Whole-graph legacy replacement, static-link guard, graph slice guard, sorted
  graph/shared-list guards, eight-category binary inventory, sanitizer harness,
  Windows diagnostics and Python invocation guards pass.
- Soldr rustfmt and `git diff --check` pass.
- Test cost: 25 reduced debug registration runs, longest 0.047803 s, peak child
  RSS 15,292 KiB, no own threads. Runtime compilation retains the existing named
  native-build budget and compiler/CMake subprocesses.

Evidence artifacts: RED `local-test-AwqBwp.log`; first GCC runtime
`local-test-e9dUS6.log`; Clang `local-test-74ZaG7.log`; scoped Clippy
`local-test-0oAsoe.log` (exit 0); core guards `local-test-z3fPGm.log`;
whole-graph `local-test-kng8tm.log`; diagnostics/Python guards
`local-test-dTGD9u.log`. Logs are under the session's private tool-output
`local-test-private` directory. Initial failures from missing explicit OS facts
in the new check and a missing host cmocka pkg-config path were corrected before
these passing runs.

Remaining candidates retain the preceding slice's deferrals: 12 need additional
libobs/SIMDe/config-header contracts, 13 need real-core/private-adapter portable
build contracts, 9 need real-core linkage/header contracts, 2 need further
OS/sanitizer contracts (window matching and Windows keys; the latter includes
existing link stubs), and 1 requires graphics/display runtime. This slice does
not classify those contracts as impossible, and does not simplify them to add
registrations.

Not verified: native macOS/Windows runtime, final-head hybrid/C-reference OFF/ON
Tier 2 and export parity, full workspace lint/unittest, application/profile or
protocol behavior, or hosted CI. These remain rollup admission work. No PR is
opened or auto-merge armed for this branch-only handoff; #1390 remains open.
