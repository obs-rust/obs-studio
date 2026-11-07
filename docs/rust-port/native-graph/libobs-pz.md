# Batch 2: real-core SCK and VAAPI helper tests

Refs #1390. Based on the native-graph rollup (#3832); this slice registers
`test_sck_stall`, `test_sck_stop_wait` and `test_vaapi_probe_policy`.
The original C sources and public headers are unchanged. Each executable
links real `OBS::libobs` and cmocka, with both original sources and its
component include directory. The stop-wait target also declares the Windows
pthread interface needed by `util/threading.h`. Headers and owning PCH files
are tracked as rebuild inputs.

The SCK helpers remain unguarded; VAAPI probe policy retains its Linux,
FreeBSD or OpenBSD guard on both target and test. All three keep a direct
one-argument command and no timeout, wrapper, environment override or skip
return code. No Rust executable or workflow is added. The registration
module belongs to the existing Application category binary.

## Scope and exclusions

- The five pipe targets are already registered in the base.
- `test_qsv_host` belongs to #3830 and is explicitly excluded.
- `test_qsv_encode_bounds` and `test_qsv_update` need the VPL SDK and remain
  deferred; their original tests supply their own scripted encoder doubles.
- Replay-buffer naming, slideshow manual end, scroll-filter wide source,
  scene size race, startup allocation stability, RTMPS upgrade, raw-output
  video reset and Pulse default-follow need display/core startup or graphics.
- PipeWire/Pulse device paths, V4L2 devices and host-only capture paths remain
  outside this headless helper slice. Scripting, Qt and plugin-loading suites
  require their original additional runtime dependencies.
- Other p–z candidates remain follow-up work; this is not an exhaustive
  completion claim for that range or for #1390.

## Validation

RED commit `995de9949` runs the focused registration check and fails because
`test_sck_stall` is absent from the canonical graph (Soldr log
`20261107T062307Z-home-niteris-dev-obs-rust-obs-studio4.log`). GREEN checks pin
the exact complete target and test records and evaluate all five OS facts.

The opt-in Linux runtime contract compiles and runs these unchanged sources
through both the Cargo graph and CMake consumer using installed real libobs
and cmocka packages. Its CTest inventory asserts exactly the three names,
direct commands and absent timeouts. Installed OBS 32.1.2 is an external-core
smoke check, not final-head hybrid/C-reference parity or export-parity
validation. Native Windows/macOS runtime and current-core OFF/ON Tier 2
remain required in the manager's combined rollup validation. No protocol or
application behavior changes are made, so no new Tier 4 scenario applies;
existing Rust unit/oracle differential tests remain intact.

Focused exact registration and whole-graph legacy replacement pass. The
GCC actual Cargo/CMake contract passes both Rust checks and all 16 unchanged
C cases (7 stall, 4 stop-wait, 5 probe policy). Graph/sorted-list/slice,
CMake-facade, Tier 2 concurrency and build-group guards pass (22 tests).
Scoped Application-test Clippy with `libobs-integration` passes. The category
inventory remains eight binaries. One primary read-only `clud-review` is clean.

Test cost: 25 reduced/debug registration observations, longest 0.450 s,
peak child RSS 14,576 KiB, no test-owned threads. The native compilation
replay uses the existing named compilation-budget exception and CMake/compiler
subprocesses; the unchanged stop-wait case waits 25 ms on a real event.
