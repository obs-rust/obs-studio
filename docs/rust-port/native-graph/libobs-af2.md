# Portable capture helpers (batch 2, libobs-af2)

Refs #1390. Continue `test/1390-b2-libobs-af`, preserving its two scalar
filter targets and history. This slice adds three unchanged executables:
`test_coreaudio_retry` (five cmocka cases), `test_coreaudio_zero_channels`
(four), and `test_display_reinit` (eight). They exercise production portable
capture helpers without OBS startup, a display, Apple SDKs or capture devices.
The retry test uses real libobs allocation, events and atomics.

Preserve unconditional selection, real `OBS::libobs` and cmocka linkage,
mac-capture include paths, the display implementation source, direct test
commands, and absent timeouts/wrappers. Watch mac-capture helper inputs.
Declare Windows-only `OBS::w32-pthreads` for the retry header's
`util/threading.h` dependency; no new package discovery is introduced.
Existing C tests, public headers and WebSocket sources remain unchanged.

RED `f2411feda`: the existing Application category registration check fails
with `missing portable capture helper`. GREEN registration parses the actual
graph and checks Linux, macOS, Windows, FreeBSD and OpenBSD selection.
Whole-graph legacy replacement and focused graph/sorted-list/slice/build-group
Python guards pass. Focused application/build Clippy passes.

GCC and Clang each compile, link and run all three targets through actual
Cargo graph and CMake consumers against installed real OBS 32.1.2. Each
consumer/compiler combination passes all 17 cmocka cases. Cargo loader checks
confirm the selected libobs. Both CTest inventories have exactly three direct
commands and no timeout. Evidence: `/tmp/parity-b2-libobs-af2/{gcc,clang}-{0,1,2,3}.log`.
This is external-library smoke evidence, not current-head hybrid/C-reference
runtime or dynamic export parity; those remain required rollup validation.
Native Windows/macOS runtime and broader application/protocol/profile paths
were not run. No PR or hosted CI was requested by this branch-only assignment.

Test cost: 25 reduced debug registration observations, longest 0.18 seconds,
peak RSS 11,692 KiB, no test-owned threads. No Rust executable is added.

Skipped: parent-branch parser blocks/functions/params/passes/shader-call/writer
are already registered, while their Rust-static companions belong to the
separate Rust-static slice. `test_async_delay_pacing` belongs to #3829.
Audio/video startup, graphics rendering and device capture cases retain their
original display/hardware requirements. Qt/frontend and FFmpeg/file-updater
integration cases are deferred; this bounded addition makes no exhaustive
eligibility or completion claim for them.
