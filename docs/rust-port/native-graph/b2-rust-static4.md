# Batch 2: Rust-static source and scene header decision tests (#1390)

This continuation branches from `test/1390-b2-rust-static3`, retaining all
prior registrations. Ten unchanged archive-only C executables are shared by
the native Cargo and CMake graph:

- `test_async_upload_steps_rust`
- `test_deinterlace_draw_plan_steps_rust`
- `test_scene_render_plan_steps_rust`
- `test_source_audio_place_steps_rust`
- `test_source_draw_plan_steps_rust`
- `test_source_media_admission_steps_rust`
- `test_source_monitoring_steps_rust`
- `test_source_render_route_steps_rust`
- `test_source_video_tick_steps_rust`
- `test_transition_load_steps_rust`

Each retains its original C source, libobs header interface, archive-first and
cmocka-second linkage, original conditional math library, direct command,
absent timeout/skip/wrapper policy, and archive-availability guard on target
and test. In particular, scene rendering retains `UNIX` for math, while six
other cases retain `NOT MSVC`; three have no explicit math dependency.
SIMDe is an explicit header dependency using the existing portable imported
target, without introducing a libobs core link or a pkg-config-only lookup.
The unchanged tests call pure decision shims: no graphics context, display or
hardware is needed despite their render-related names. No C tests, public
headers, Rust implementations or WebSocket code are edited.

An independent registration module in the existing Application category checks
sources, include/header interfaces, link order and math guards, empty policy
fields, direct commands, absent timeouts and archive presence on Linux,
macOS and Windows. The feature-gated Linux runtime contract builds and runs
all ten through the real Cargo graph and CMake consumer with the production
Rust archive. The header-only fixture configures the production
`libobs/obsconfig.h.in` template and finds actual SIMDe with the repository's
finder. It provides no replacement core or symbol stubs. Cargo consumes the
same configured header and real SIMDe include directories. CTest inventory is
checked with the archive disabled and enabled, including absent TIMEOUT
properties. No new Rust executable or workflow is added.

RED: `68edc942b`, direct graph inventory assertion fails for all ten missing
Rust-static entries (exit 1, `local-test-AhMGTV.log`). GREEN registration and
real runtime validation are recorded in the worker response. The initial
Soldr porcelain run returned only metadata; it is not claimed as RED evidence.
Explicit `SOLDR_PORCELAIN=0` and the repository-prescribed Rust wrapper bypass
were used for subsequent compiler diagnostics. Initial fixture compile/import
and missing local SIMDe search-path errors were fixed before passing runs.

Local evidence is archive-only Linux evidence, not final-head hybrid versus
C-reference parity. Native macOS/Windows runtime, broad OFF/ON Tier 2 and exact
export parity, full workspace lint/unittest, application/profile/protocol
behavior and hosted CI remain rollup admission work. No PR or auto-merge is
created for this branch-only assignment; #1390 remains open.

Remaining candidates: 27 total: 2 additional header/host contracts
(`test_pulse_monitor_steps_rust`, `test_transition_start_steps_rust`),
13 real-core/private-adapter contracts, 9 real-core linkage/header contracts,
2 additional OS/sanitizer contracts (window matching and Windows keys, the
latter with existing link stubs), and 1 graphics/display runtime case. The two
remaining header cases are deferred at the ten-test assignment cap; the other
categories retain the preceding slice's scope, not a claim of impossibility.

Validation evidence (private `local-test-private` logs):

- Final GCC Cargo+CMake replay: `local-test-MSHZUA.log`; Clang:
  `local-test-RAppmU.log`. Ten executables through each consumer per compiler.
  Existing missing-field-initializer C warnings remain unedited.
- Whole-graph legacy replacement: `local-test-OqAGYM.log`.
- Static-link, graph slice, graph/shared-list sorting, eight-category binary
  inventory, sanitizer harness and Python invocation guards:
  `local-test-3g5yAB.log`; Windows diagnostics: `local-test-aVOzsY.log`.
- Scoped Application-test Clippy with `libobs-integration` passes (exit 0;
  `local-test-QzCSEv.log`); no complexity allowance or threshold change.
- Test cost: 25 reduced debug registration runs, longest 0.033182 s, peak
  child RSS 10,616 KiB, no own threads. The real native compilation contract
  retains the existing named build-budget exception and compiler/CMake
  subprocesses.
