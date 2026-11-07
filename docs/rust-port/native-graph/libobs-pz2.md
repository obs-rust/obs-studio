# Batch 2 continuation: direct video scaler

Refs #1390. Branches from `test/1390-b2-libobs-pz`, preserving its three
registrations. Adds the unchanged `test_video_scaler_direct` executable with
real `OBS::libobs` and cmocka linkage. The original Linux-only target/test
guard, direct executable command and 120-second timeout are preserved.
No C test, public header, WebSocket source or workflow changes. Registration
and opt-in compilation checks join the existing Application category binary.

## Scope

The three original cases compare visible bytes for aligned and odd-stride
BGRA-to-NV12 frames at 1080p, 4K and 4K-to-1080p. They use the real CPU video
scaler without OBS startup, graphics, a display or devices. Performance
numbers printed by the unchanged test are diagnostic; this graph change
makes no speed claim.

`test_pipewire_frame_pacing` and `test_pulse_timing` are headless candidates
but their owning PCHs include the full plugin development headers. Required
PipeWire/Pulse development packages were not available to this slot's
pkg-config search. Both are deferred without changing their guards or
supplying doubles. Other p–z runtime/SDK cases remain as described in the
parent slice; this continuation does not claim the range is exhausted.

## Validation contract

RED `9a4f001c3` fails the exact registration check with a null target record
(Soldr log `20261107T063802Z-home-niteris-dev-obs-rust-obs-studio4.log`). The
GREEN check pins the complete target/test records and Linux selection versus
macOS, Windows, FreeBSD and OpenBSD exclusion. The opt-in real-library
contract compiles and runs the unchanged source through Cargo and CMake,
then verifies the exact single-test CTest inventory, direct command and
120-second timeout.

Installed OBS 32.1.2 smoke evidence is distinct from current hybrid and
C-reference OFF/ON Tier 2 and exact export parity. Those remain manager
rollup gates, together with native Windows/macOS configuration/runtime.
No behavior or protocol implementation changes; existing Rust unit/oracle
differential assertions remain intact and no new Tier 4 scenario applies.

GCC and Clang each pass both focused Rust checks and all three unchanged C
cases through each real Cargo/CMake path. CTest inventory confirms exactly
one direct executable and TIMEOUT=120. Forty-six Python graph-slice,
CMake-facade, binary-budget, build-group and Tier 2 concurrency guards pass.
Scoped Application Clippy with `libobs-integration` passes. Repository lint
passes its earlier lanes then exits 1 at Soldr's documented Dylint-driver
source-build refusal under clud; no workaround was attempted. The full
unit-test wrapper is not run because this assignment forbids target pruning.

Evidence in clud's `tool-output/local-test-private/`: `local-test-xHpDuP.log`
(GCC), `local-test-UGPLPU.log` (Clang), `local-test-GbN1fG.log` (33 guards),
`local-test-DRz9pT.log` (13 guards), `local-test-lNEyvU.log` (Clippy),
`local-test-TZ631f.log` (repository lint limitation). The eight-category
Rust test executable inventory remains unchanged. This opt-in native compile
contract uses the existing native-build time-budget exception (#2888).

Whole-graph legacy replacement passes (`local-test-CYbt2g.log`); scoped
rustfmt check passes. Test cost: 25 reduced debug registration observations,
longest 0.086 s, peak child RSS 14,808 KiB, no test-owned threads
(`local-test-MNOEE6.log`). No new generated allocations or threads are added;
the unchanged C scaler retains its fixed frame sizes and iteration counts.
