# Headless image EXIF and media remux graph slice (#1390)

This slice continues `test/1390-b2-libobs-go3` at
`60d891b0e53568814c77931e2a0211c7e267ffa3`. It registers two unchanged
real `OBS::libobs` executables: `test_image_exif_orientation` (27 cases)
and `test_media_remux` (13 cases). It does not complete #1390.

Both targets retain their original source and link order: libobs, FFmpeg
avcodec, avformat for remux only, avutil, cmocka. Both remain unconditional
on all supported hosts, run directly and retain their 60-second timeouts.
No display, graphics module or hardware is required. EXIF generates images
and decodes on the CPU; remux writes WAV inputs without an encoder.
No C tests, public headers, WebSocket sources or workflows change.

The existing Application category gains one registration module, with exact
target/runtime assertions and five-host selection checks; no Rust executable
is added. RED `a7ce8a547` requires the absent registrations. The Rust check
failed and then passed with the graph records and legacy replacement.
The independent focused missing-entry check also failed before the change
and passed afterwards.

## Local evidence and limits

- Actual Cargo graph and canonical CMake consumer configure/build replay with
  GCC and Clang links both unchanged executables to installed OBS 32.1.2.
  `ldd` verifies the selected library for both Cargo artifacts per compiler.
- Remux passes all **13** cases through Cargo, canonical CMake and original
  CMake with both compilers. EXIF passes six and fails **21** orientation
  cases with that old external core through all three consumers and both
  compilers. Original CMake and canonical CMake each return CTest exit 8;
  the Cargo replay returns 101 after recording the test exit 21. No assertions
  are weakened, retries added or failures marked successful. This is matching
  external-core smoke evidence, **not GREEN final-head runtime parity**.
- Original/canonical CMake inventories agree for Linux, Windows, macOS,
  FreeBSD and OpenBSD: two tests, timeout 60, no skip-return or resource
  overrides. These are configure/selection simulations, not native host runs.
- Focused Application registration passes; scoped Application/obs-build
  Clippy with `libobs-integration` and `-D warnings`, scoped rustfmt, binary
  budget, sorted graph, native slice and sanitizer harness guards pass.
- Test cost: 25 reduced debug observations, longest
  0.025129 seconds, peak RSS 10788 KiB,
  no test-owned threads.

Replay sources, original CMake fragments, per-executable observations,
consumer exit codes, inventory and cost results are in `/tmp/b2-libobs-go4/`.
Private bounded reports are under
`/home/niteris/.clud/tmp/tool-output/local-test-private/`: RED
`local-test-eAGVMN.log` (Rust), `local-test-FoKn5F.log` (independent check);
GREEN registration `local-test-DqrLup.log`; compiler/consumer replay
`local-test-tGbWYj.log`; inventories `local-test-j7W5vK.log`; cost
`local-test-J0jq4b.log`; Clippy `local-test-OYmWMz.log`; guards
`local-test-blb6RZ.log`; formatting `local-test-ZqYmh7.log`.

Current-head hybrid/C-reference runtime and exact export parity, native
Windows/macOS execution, and full application/profile/protocol/hardware paths
remain unverified. No PR or hosted CI was requested for this branch-only
rollup handoff. No target pruning or soldr GC was run.
