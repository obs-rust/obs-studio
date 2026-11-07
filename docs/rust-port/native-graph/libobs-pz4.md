# Batch 2 continuation: PulseAudio monitor format

Refs #1390. Continues `test/1390-b2-libobs-pz3`, retaining all five prior
registrations. Adds `test_pulse_monitor_format` with both unchanged sources,
real OBS::libobs, PulseAudio::PulseAudio and cmocka linkage. The original
Linux/FreeBSD plus ENABLE_PULSEAUDIO guard applies to target and test;
the optional package lookup stays under that guard. The graph retains
address,undefined sanitizers, the direct executable command and no timeout.
The local libobs include directory resolves the production PCH; the PCH and
helper header are watched for rebuilds. No C tests, public headers, WebSocket
sources or workflows change. No Rust executable is added.

## Scope

All 12 original cases cover formats, channels, channel maps, integer/float
volume processing, frame counts, queued bytes, drop splitting and device
matching. They need no display, OBS startup, server or hardware. They compile
the production C helper, so they characterize C rather than a Rust swap of
those helper symbols. Existing Rust unit/oracle differential coverage remains.

The real-library opt-in compilation contract uses the existing Application
binary and named native-build budget exception. It compiles and runs the
selected graph through Cargo and CMake; the latter reuses the production
sanitizer helper extracted from test/cmocka/CMakeLists.txt and the unchanged
PulseAudio finder. It verifies exactly one CTest entry with a direct command
and no timeout. Production discovery has no added platform-specific lookup.

Deferred candidates remain those in the parent slices: display/graphics or
core startup paths, device/daemon and plugin SDK suites, Qt/scripting/plugin
loading, native host-only paths and QSV host owned by #3830. PipeWire pacing
and Pulse timing need full owning plugin PCH dependencies; this smaller
libobs PCH compiles with the available PulseAudio development package.
This is not a claim that every p-z candidate is exhausted.

## Validation

RED `ebcd61b00` fails with the missing target record (local-test-f9BUHO.log).
The exact registration check now passes, including ENABLE_PULSEAUDIO on/off
for Linux, FreeBSD, OpenBSD, macOS and Windows. GCC and Clang each pass
both registration and actual Cargo/CMake compile/run contracts, including all
12 unchanged C cases through each path with ASan/UBSan enabled
(local-test-3vEqiJ.log and local-test-i5vTkw.log).

Thirty-three focused Python graph-slice, sorted-list, CMake compatibility,
binary-budget, build-group, Tier 2 concurrency and sanitizer guards pass
(local-test-Dc7Koj.log). Whole-graph legacy replacement passes
(local-test-OBuD5N.log). One primary read-only clud-review reports clean.
An initial scoped Clippy attempt was refused by Soldr's active macOS-target
build lock; no lock was changed. A later attempt found a single-element
inventory loop, which was simplified. Final scoped Application Clippy with
libobs-integration passes (local-test-8YjLml.log); scoped rustfmt and diff
checks pass.

Test cost: 25 reduced debug exact registration runs, longest 0.096 seconds,
peak child RSS 14,936 KiB, no test-owned threads (local-test-AVZlHu.log).
The native replay uses compiler/CMake subprocesses and the existing compile
budget exception. Evidence artifacts are under clud tool-output/local-test-private.

Installed OBS 32.1.2 is external-library smoke evidence only. Final-head
hybrid/C-reference OFF/ON Tier 2 and exact export parity remain the manager's
combined rollup gates. Native Windows/macOS runtime is not run here. No
protocol or application behavior is changed, so no new Tier 4 scenario is
introduced. No full lint/unittest, target pruning, soldr gc, PR or hosted CI.
