# FFmpeg packet write native graph (batch 2, libobs-af4)

Refs #1390. Continue `test/1390-b2-libobs-af3`, preserving its history.
Register unchanged Linux `test_ffmpeg_mux_write_packet` with its production
mux implementation and POSIX host group, real OBS::libobs, FFmpeg avformat
and avutil, cmocka, pipe-write linker wrapper and 60-second timeout.
Its three cases check the small packet, 64 KiB boundary and large packet paths
without OBS startup, a display or hardware. The original C test and public
headers remain unchanged; no Rust test executable is added.

RED `11996ee46` adds the Application-category registration check. A local
missing-registration assertion fails with `missing real-libobs packet write
test` (local-test-KIfGJc.log). The first Rust execution overlapped the GREEN
edit and instead detected an invalid timeout schema field; this was corrected
to `timeout_seconds`. No remote RED run or PR was requested.

GCC and Clang each compile/link/run the three unchanged cases through both
actual Cargo graph and CMake consumers, against installed OBS 32.1.2.
Loader checks confirm the selected real libobs; both CTest JSON inventories
have one direct command with timeout 60. Evidence: local-test-aVfg4u.log;
replay harness and CTest inventories: `/tmp/parity-b2-libobs-af4/`.
This installed-library smoke evidence does not establish final-head hybrid /
C-reference runtime or exact export parity, which remain required rollup
validation. Native Windows/macOS runtime and broader app/profile/protocol
paths were not run. The registration check verifies Linux selection and
exclusion on macOS, Windows, FreeBSD and OpenBSD.

This is a bounded single-target continuation. ALSA null-device wrapper tests,
other FFmpeg helpers and file-updater tests remain unconverted candidates,
not declared ineligible. Display-dependent audio/video, graphics and Qt tests
remain deferred. Rust-static tests belong to their assigned slice;
async-delay pacing remains owned by #3829. No PR, auto-merge, hosted CI,
target pruning or soldr gc is requested by this branch-only assignment.
