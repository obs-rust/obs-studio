# Batch 4 follow-up: headless V4L2 capture SDK contracts

Refs #1390. Branch-only pool slice based on `test/1390-b4-sdk`
(`3e3e44b9ba213efca94d2123b6d54282c17b3f49`). No PR or hosted CI;
the pool manager owns the combined rollup gate.

| Unchanged executable | Cases | Timeout |
| --- | ---: | --- |
| `test_v4l2_controls` | 32 | 60 seconds |
| `test_v4l2_corrupt_frame` | 10 | 60 seconds |
| `test_v4l2_dqbuf_error` | 7 | 90 seconds |
| `test_v4l2_framerate` | 11 | 60 seconds |

All four preserve their original Linux/`OBS::v4l2` guard, complete source
lists, libobs/FFmpeg/cmocka linkage and compile definitions. They supply their
original fake camera drivers and need no camera, daemon or display. No C test,
public header, WebSocket plugin, workflow or Rust test executable changes.
The graph adds four targets and four tests, 511 → 515, leaving existing
records unchanged.

The shared SDK helper uses QUIET discovery only inside the Linux/owner guard.
Missing libv4l2 headers or any of the three FFmpeg imported targets disables
the group. Header-only libv4l2 preserves the original test's driver definitions;
FFmpeg retains real linkage. The main Cargo builder still does not build the
V4L2 owner, so its new SDK fact defaults false.

## Evidence and limitations

RED `2ca330c82`: the registration contract fails on the absent controls graph
entry (`local-test-VI18z9.log`, expanded diagnostic in Soldr's
`20261107T094537Z-home-niteris-dev-obs-rust-obs-studio2.log`).
The initial RED candidate set also contained `test_v4l2_fd_cloexec`.
Its first case failed with the installed OBS 32.1.2 process launcher retaining
an inheritable descriptor (`local-test-5oBBw8.log`, assertion at unchanged
`test_v4l2_fd_cloexec.c:349`). That executable remains on its original CMake
path, unchanged and outside this slice. No retry or assertion weakening.
It needs final-head dual-core validation rather than the installed-core smoke
fixture used here.

The original cache-wrapped native Rust build stalled; it was interrupted
(`local-test-N0o3Kf.log`). The documented `SOLDR_RUSTC_WRAPPER=none` replay
reached the native tests. No shared daemon restart or cache purge.

Detailed final validation is recorded in the pool response file. Installed-core
runtime results do not establish final-head hybrid/C-reference parity, exact
exports, application/profile behavior or native Windows/macOS/BSD execution.

GCC registration, guard, package-free configure and runtime contracts pass
(`local-test-xSPe8D.log`): 60 unchanged native cases through Cargo/cc::Build,
then the same 60 through CMake/CTest. CMake inventories also preserve the
60/90-second timeouts and select no tests on non-Linux/BSD, with the owner
off, with FFmpeg disabled or with optional pkg-config discovery disabled.
Package-free configure covers Linux, FreeBSD, OpenBSD, Windows and macOS.
Clang replays the same Application harness with `CC=clang CXX=clang++`;
all four contracts and the same 60 cases through each native consumer pass
(`local-test-ef6xAB.log`).
Nine focused Python graph/portability/cmocka/budget checks pass
(`local-test-dm8sWi.log`). Gersemi 0.25.0 checks all five changed CMake files.
One primary clud-review reviewer reports clean, including the narrowed scope.
The whole-graph legacy replacement contract also passes
(`local-test-9xhb3t.log`).

Full `SOLDR_PORCELAIN=0 SOLDR_RUSTC_WRAPPER=none ./lint` ran before push
(`local-test-bkefuY.log`). Its formatting and gersemi steps passed, but the
Python self-test phase failed in unrelated
`test_resource_samples.py::test_samples_are_off_on_request`: a resource
sample appeared despite the off setting. The exact test file passes in an
isolated run (`local-test-bdEqGK.log`). This does not turn the full lint run
green; its later Rust/Dylint/feature-matrix steps remain unverified by that
run. No full-gate retry, workaround or attestation stamp.
Separate scoped all-target Clippy with `libobs-integration` and `-D warnings`
passes on Linux (`local-test-yU6kyk.log`) and `x86_64-pc-windows-msvc`
(`local-test-97mnof.log`). Windows is compilation/lint evidence, not execution.
The Rust category inventory remains eight binaries; no complexity allowance
changes.

Test cost, 25 reduced-debug runs each: registration longest 0.0448 s;
guards 0.0171 s; SDK-absent configure 0.7930 s; peak child RSS 19,324 KiB.
These fast contracts own no threads. The unchanged native tests retain their
capture threads; real compiler/CMake subprocess replay retains the existing
native-build budget exception. Measurements are in
`build_ci_audit/b4-sdk2/test-cost.json`.

## Deferred inventory

The original 41 SDK/owner fragments now have nine registrations across the
base and these two batch-4 slices, leaving 32 deferred. Seven retain excluded
runtime requirements; two PipeWire connect-start/teardown-wait candidates need
the real linked GLAD/GIO fixture; one V4L2 descriptor-inheritance candidate needs
final-head core runtime validation; 22 others remain unconverted, including
V4L2 busy-signal/device identity/virtual devices, FreeType font fixtures,
Wayland/xkbcommon, Qt, Python, QSV/VPL and VST. This is a focused slice, not an
exhaustion claim. The 40 original graphics/WebSocket/display exclusions remain
outside scope. No target pruning, garbage collection or slot reuse.
