# Batch 2 continuation: audio remux timestamps

Refs #1390. Based on `test/1390-b2-libobs-pz2`, retaining its four additions.
Registers the unchanged `test_remux_audio_timestamps` with real `OBS::libobs`,
FFmpeg avcodec/avformat/avutil and cmocka linkage. The original unconditional
selection, non-Windows math library, direct executable command and 60-second
timeout are preserved. Existing C tests, public headers, WebSocket, workflows
and the eight Rust category executables are unchanged.

The four original cases encode AAC into Matroska and check sample-accurate
remuxing to MP4/MOV, a trimmed final frame and a real recording gap. No OBS
startup, display, graphics, hardware or server is required. Source-defined
codec-unavailable skips remain unchanged; no skip or retry was added.

## Evidence and limits

RED `76dd3f63f` fails on the missing target record (`local-test-4jxiOe.log`).
GREEN registration pins complete target/test records and selection on Linux,
macOS, Windows, FreeBSD and OpenBSD. Separate opt-in Cargo and CMake contracts
compile and run the original source, retaining every assertion. CMake also
checks exactly one CTest entry, a direct command and TIMEOUT=60 before running.

Installed OBS 32.1.2 is an external smoke dependency, not the current hybrid or
C-reference core. Both compiler replays build the unchanged source, but all
four cases fail on that installed core's audio timestamp assertions. These
failures are retained and must not be reported as runtime GREEN. The manager
must run current-head OFF/ON Tier 2 and exact export parity in the combined
rollup. Native Windows/macOS runtime is not exercised here. Existing Rust
unit/oracle differential assertions remain intact; no new protocol behavior
or Tier 4 scenario is introduced.

Clang evidence: `local-test-n7i8f3.log`; GCC evidence:
`local-test-UJmnkw.log`, replayed through the already-built Application binary.
Soldr refused the GCC Rust rebuild while another macOS-target build lock was
active (`local-test-TN5RQw.log`); no lock was removed and no other build was
stopped. All log names refer to clud's `tool-output/local-test-private/`.

Whole-graph legacy replacement passes (`local-test-9rSBLb.log`). Thirty focused
Python graph-slice, libobs CMake-facade, binary-budget, build-group and Tier 2
concurrency guards pass (`local-test-iZJODt.log`). Scoped Application Clippy
with `libobs-integration` passes (`local-test-k9Iexn.log`); scoped rustfmt and
diff whitespace checks pass. Full lint/unittest and final-head dual-core
validation are not claimed. No cache pruning or soldr gc was run.

Registration cost: 25 reduced debug observations, longest 0.155 s, peak child
RSS 14,836 KiB, no own threads. Native compilation checks use the existing
named native-build budget exception and compiler/CMake subprocesses.

Remaining p–z display/device/Qt/scripting/platform cases remain deferred as
described by the parent slices; QSV host remains owned by #3830. The pacing
and Pulse timing helpers still need their full owning PCH SDK environment.
This slice does not claim the range is exhausted.
