# Batch 3: headless plugin SDK cases

Refs #1390. Based on `test/1390-b3-c` (`768d55b5d`), excluding all its
registrations. Adds two unchanged Linux/plugin-guarded native executables:

| Test | Cases | Original timeout | Original linked dependencies |
| --- | --- | --- | --- |
| `test_pulse_default_retry` | 4 | 10 seconds | real libobs, PulseAudio, cmocka |
| `test_pipewire_frame_layout` | 19 | unset | real libobs, PipeWire, cmocka |

Both original `OS_LINUX AND TARGET OBS::<owning-plugin>` guards remain.
Their sources, direct commands, linker wrappers and timeout behavior are
preserved. Neither needs OBS startup, a daemon, a device, a display or an
OpenGL implementation. Existing C tests and public headers are untouched.
No Rust test executable is added; contracts belong to the Application binary.

The Pulse test retains the real production event/restart/timer path and its
three original linker-wrap options. The graph expresses the same linker
arguments with the GNU driver spelling `-Wl,` rather than CMake-only
`LINKER:`, so both consumers forward them correctly. PipeWire retains the production
formats/frame-layout implementation and PCH. That PCH includes DRM, GIO/GLib
and GLAD headers even though the tested code does not call GL: the graph
explicitly declares `libdrm` and `gio-2.0` header packages and the repository's
GLAD include directory. These add headers, not replacement implementations
or extra linked libraries.

Optional SDK facts default false. CMake discovers the SDKs quietly inside
the original platform/plugin guards and selects each entry only when its
SDK is available. Cargo derives matching facts from its resolved PulseAudio/
PipeWire interfaces and optional DRM/GIO probes. No new SDK lookup runs on
Windows or macOS. Missing optional SDKs exclude the associated entry; they
do not introduce a configure failure. Existing core SDK requirements remain.
The legacy batch-2 inventory check keeps these two facts false for its
original count, then explicitly checks that enabling both adds two entries.

The real-library smoke fixture imports the installed owning plugin binaries
to exercise the original target guards, without loading them. It uses the
checkout's public headers, as the production build does, and a real installed
OBS library. This matters for the new async-compensation declaration wrapped
by the unchanged Pulse test: installed OBS 32.1.2 headers predate it. The
fixture adds no declaration or function stub. Both replay consumers retain
OBS's hidden-symbol policy (`compiler_common.cmake` and Cargo's existing
`configure_c_tests` interface); bare pkg-config dependency import alone does
not carry that policy. The optimized CMake control passes; the initial
Cargo smoke without that inherited visibility policy does not link. Installed-library smoke is
separate from final-head hybrid/C-reference execution and exact export parity.

## Scope left for later slices

Within the original 41-fragment owning-plugin/SDK group, two are registered
here and 39 remain:

- Seven forbidden runtime fixtures: Pulse default-follow/monitor-device/
  monitor-reset and PipeWire camera-registry-race/camera-unload/camera-unplug/
  context-fail need a display and/or a real private sound-server/portal daemon.
  The context-fail control case starts a daemon, so its wrapper stays intact
  and the whole test remains excluded.
- Two further headless candidates, PipeWire connect-start and teardown-wait,
  need their original real GLAD/GIO linked dependency contract in addition to
  the header-only closure handled here; deferred, not declared impossible.
- Thirty other plugin/SDK fragments are outside this focused Pulse/PipeWire
  slice (Qt/plugin runtime, Python, QSV/VPL, FreeType, V4L2, VAAPI, Wayland).

The other 40 fragments from b3-d remain outside this assignment's runtime
scope: 30 graphics fixture, seven WebSocket, two X11 display and one real
services-plugin/display. WebSocket files and submodule are untouched.

## Validation

RED `24783afc2`: both registration checks fail because the graph omits the
entries (`local-test-gWAtyx.log`). GREEN records are checked for exact sources,
linkage, headers, guards, wrappers and timeout. Selection covers all Boolean
combinations of Linux, owning plugin and SDK availability. A package-free
CMake contract configures with all three optional package finders disabled,
with Linux both on and off, and confirms neither executable is selected.

Runtime contracts compile and run unchanged cases through Cargo/cc::Build
and CMake/CTest. Additional CMake configurations check Linux-off, owner-off,
PulseAudio-absent, PipeWire-absent and DRM/GIO pkg-config-absent selection.
A real empty pkg-config search directory, with CMake prefix injection disabled
and SDK wrappers' role/target-specific package paths cleared, models missing header SDKs without
fake packages, fake headers or changed tests. The first GCC smoke attempt failed against the installed old headers
(`local-test-tnqZog.log`, matching CMake failure `local-test-QfJe5m.log`);
using the checkout headers fixes that dependency mismatch. Evidence and
final status are recorded in the pool response.

Final-head hybrid/C-reference Tier 2, exact exports, Windows/macOS native
execution, full lint/unittest and hosted CI are not claimed. The manager's
combined rollup owns those gates; this assignment opens no PR or hosted run.
No target pruning or soldr garbage collection is performed.

Final GCC and Clang replays each pass all four Rust contract checks and all
23 unchanged native cases through **each** real graph consumer, including
the six enabled/disabled CMake configurations. Evidence: `local-test-SXnJK8.log`
(GCC) and `local-test-ozuVjz.log` (Clang). The same built Application harness
was replayed with `CC=clang` for Clang, avoiding an unrelated Rust rebuild;
its native Cargo/cc::Build and CMake compiler invocations use Clang.
Final scoped Clippy with `libobs-integration` passes (`local-test-gCntD1.log`).
Gersemi 0.25.0, scoped rustfmt, graph-slice/sorting/compatibility/budget guards
and `git diff --check` pass. The canonical graph has 508 targets/test entries.

Test cost: 25 debug runs per fast Rust contract, longest 0.0741 seconds for
registration, 0.0776 seconds for guard selection and 0.2802 seconds for the
package-free configure check; peak child RSS 19,248 KiB. These contracts and
the unchanged native cases own no threads. The runtime contract launches
compiler/CMake subprocesses under the existing native-build budget exception.
The fixed-input cases do not sample parity inputs. Measurements are retained
in `build_ci_audit/b3-e-sdk/test-cost.json`.

The whole-graph legacy replacement contract also passes (`local-test-zo6Htt.log`);
its explicitly enumerated unavailable-SDK facts now include both new false
SDK defaults. One primary clud-review reviewer found no remaining blockers.
