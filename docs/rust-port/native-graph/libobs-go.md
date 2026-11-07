# Batch 2: real libobs, g–o

Refs #1390. Based on #3832's native-graph rollup. This is a bounded
five-executable slice, not completion of the g–o inventory or parity work.

| Unchanged executable | Cases | Original selection | Timeout |
| --- | ---: | --- | --- |
| `test_graphics_host_exports` | 2 | Linux | none |
| `test_graphics_host_functions` | 1 | Linux | none |
| `test_image_source_host` | 4 | neither Windows nor macOS | none |
| `test_ingest_refresh_wait` | 6 | unconditional | 60 s |
| `test_nvenc_caps_cache` | 6 | Linux | none |

All five retain real `OBS::libobs` and cmocka linkage, original sources and
include directories, direct invocation, and absent wrappers/skip codes.
Ingest also declares `OBS::w32-pthreads` on Windows because its unchanged
threading header requires that interface. No package discovery was added.
The image-source deferred attachment for other tests remains intact.
The graphics checks inspect layouts/import lists and exported function
addresses without a graphics context. NVENC uses fake sysfs, without a GPU.

RED `fd8e47df3` fails the registration test with “canonical graph omits
test_graphics_host_exports”. GREEN registration and whole-graph legacy
replacement pass. Twenty focused Python checks pass (sorted graph, graph
slices, Windows diagnostics, CMake version and Mbed TLS graph checks).
Original and canonical CMake selection/properties match in five simulated
OS configurations: Linux has five tests, Windows/macOS one each, and
FreeBSD/OpenBSD two each, with only ingest's original 60 s timeout.

Actual GCC and Clang Cargo graph compilation/execution and CMake configure,
build and CTest each pass all five executables (19 unchanged C cases).
Each Cargo artifact's `ldd` resolution confirms the selected installed real
OBS 32.1.2 library. These are external-library smoke checks, not final-head
hybrid/C-reference pair or exact export parity evidence. Native Windows and
macOS runtime, full application/profile/protocol validation, and broader
#1390 parity remain unverified. Existing Rust unit/oracle coverage is unchanged;
Tier 4 is inapplicable to this build-inventory change.

Local artifacts: `/tmp/b2-libobs-go/` contains the replay source, inputs,
original fragments, evaluated inventories and results. Cargo/CMake replay:
`local-test-YgjGjd.log`; five-OS comparison: `local-test-LwRjKr.log`;
registration: `local-test-0xs1Hm.log`; legacy replacement:
`local-test-sFj8ZC.log`; scoped application/build Clippy:
`local-test-79adoO.log`; independent primary review:
`local-test-KvHIJM.log` (clean).

Test cost: registration, 25 reduced debug runs, longest 0.352 s,
peak RSS 10,816 KiB, no test-owned threads. Native replay uses compiler/CMake
subprocesses and the existing compilation-contract budget; the unchanged
ingest executable starts its original refresh threads.

Deferred candidates from the fragment/graph comparison:

- Happy-eyeballs tests: additional shared connector dependency, outside this bounded batch.
- Hotkey background/press-state tests: display/X11 interaction.
- GL dynamic texture, image size and output stop/join tests: graphics/display requirement.
- Other graphics host/step tests: FFmpeg, platform or Rust-static linkage beyond this batch.
- Image file/EXIF/version tests: decoding or deferred platform-host source composition, left for a separate slice.
- Inhibit tests: OS session/DBus services, beyond the selected isolated checks.
- JACK and OSS tests: audio/host integration, left for a dedicated slice.
- Lua source graphics deadlock: scripting/graphics integration.
- Media tests: media-playback/FFmpeg/runtime fixture composition, left for a dedicated slice.
- Nix startup/key steps: startup/display or distinct platform characterization dependencies.
- NSGIF: Rust-static linkage, belongs to the static worker.
- OBS outputs network/RTMP hosts: distinct host/TLS dependency composition, deferred.
- OBS startup diagnostics: X11 interposition, deferred.
- Opus mono bitrate: plugin/encoder integration, deferred.
- Output filename reservation: conditional mux/graphics composition must be preserved together.
- Output timer: Qt Widgets/AUTOMOC/AUTOUIC, deferred.

No existing C test, public header, WebSocket plugin, workflow or Rust test
executable changed. No target pruning or Soldr garbage collection was run.

Full `./lint` passes formatting, repository self-tests and workspace/feature
Clippy lanes, then exits 1 at Soldr's documented refusal to build the Dylint
v6.2.0 driver under clud for nightly-2026-05-28. Final lines were inspected
in `local-test-nXYJxG.log`. No toolchain change or workaround was attempted;
full lint is not claimed GREEN. Full unittest was not run because its wrapper
prunes targets, prohibited for this assignment. Final rollup hosted validation
remains the manager's responsibility.
