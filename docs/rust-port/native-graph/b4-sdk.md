# Batch 4: headless V4L2 and FreeType SDK contracts

Refs #1390. Branch-only pool slice based on `test/1390-b3-rollup`
(`0ea927d8e22190d290bbdfd72225b1fc58794f49`, PR #3864).

| Unchanged executable | Cases | Original guard | Original timeout |
| --- | ---: | --- | --- |
| `test_v4l2_busy` | 12 | Linux and `OBS::v4l2` | 60 seconds |
| `test_v4l2_udev_poll_error` | 3 | Linux, `OBS::v4l2`, Libudev | 60 seconds |
| `test_text_freetype2_platform` | 1 | Linux, FreeBSD or OpenBSD | unset |

The graph adds exactly these three targets/tests: 508 → 511 in each map.
Every pre-existing record is unchanged. Original C tests, public headers and
`plugins/obs-websocket` are untouched; no new workflow or Rust test binary.

The busy test's production PCH pulls in libv4l2 and FFmpeg headers. Declare
these as header packages without adding their libraries to the original
linkage. The udev test retains real Libudev linkage and its separate udev
PCH. FreeType platform defaults need FreeType and Fontconfig headers through
the production PCH, while retaining libobs/cmocka-only linkage. Discovery is
QUIET and bounded by the original OS/owning-plugin guards; missing optional
SDKs select nothing. No Linux SDK probe runs on Windows or macOS.

The main Cargo compatibility builder does not build the V4L2 owning plugin,
so its owner fact stays false, like the existing QSV/FFmpeg tests. CMake and
the explicit SDK runtime contract can enable V4L2. FreeType platform tests
have no owning-plugin guard and can run in the main Cargo builder when the
Linux/BSD header SDK is available.

## Evidence

- RED `807d6d4a1`: new registration assertion fails on the absent graph entry
  (`local-test-klvA1N.log`).
- GCC: three registration/guard/runtime contracts pass; all 16 native cases
  pass through Cargo/cc::Build and again through CMake/CTest
  (`local-test-6mS9Ie.log`).
- Clang: replay of the same freshly built Application category harness with
  `CC=clang CXX=clang++` passes the same contracts and 16 cases through each
  consumer (`local-test-LjYNZ3.log`). No unrelated Rust rebuild is needed to
  change the native compiler.
- All three fast contracts, including package-free configure, pass
  (`local-test-ew5YYb.log`). Guard selection covers Linux, FreeBSD, OpenBSD,
  Windows and macOS. Package-free CMake configure disables optional finders
  on each of those host selections, even with the owning-plugin fact on.
- Real CMake inventory: enabled = 3, non-Linux/non-BSD = 0, either BSD = 1,
  owner off = 1, Libudev unavailable = 2, optional SDKs unavailable = 0.
  Enabled inventory also checks direct commands and original timeout values.
- Six focused Python graph/portability/sorted-list/compatibility/budget
  modules pass (`local-test-nZ8TFD.log`).
- Scoped all-target Clippy, including `libobs-integration` and `-D warnings`,
  passes on Linux and `x86_64-pc-windows-msvc`
  (`local-test-PC69Ox.log`, `local-test-lqe6zI.log`); no complexity allowance
  is added. The Windows target run is compilation/lint evidence only.
- Whole-graph legacy replacement contract passes (`local-test-kfKdaw.log`).
- Gersemi 0.25.0 checks all four changed CMake files; rustfmt and diff checks
  pass. One primary clud-review reviewer reports clean.

Logs are retained under
`/home/niteris/.clud/tmp/tool-output/local-test-private/`.
Native runtime evidence uses the real installed OBS 32.1.2 SDK and module,
not final-head hybrid/C-reference parity. There are no new API or SDK stubs.

Test cost, 25 reduced-debug runs per fast contract: registration longest
0.0385 s; guard selection 0.0473 s; package-free configure 0.5692 s; peak child
RSS 19,328 KiB. Fast contracts own no threads. The unchanged udev native test
starts one worker thread at a time, the busy test starts helper processes,
and the platform-default test starts neither. Compiler/CMake subprocess
replay retains the existing native-build budget exception. Measurements:
`build_ci_audit/b4-sdk/test-cost.json`.

Full `SOLDR_PORCELAIN=0 ./lint` ran (`local-test-3YcAnG.log`): formatting,
gersemi, Python guards and the eight-category inventory passed. Workspace
Clippy was refused while a validation held the slot's build lock; the Dylint
step failed because Soldr has no deployed driver for nightly-2026-05-28 and
refuses to build it under clud. No toolchain workaround or attestation stamp.
Dylint and the full feature-set matrix remain unverified locally; the combined
rollup's hosted Fast Gate must cover them.

## Deferred scope and limits

This is a focused slice, not an exhaustion claim. From the original 41
owning-plugin/SDK fragments, the base registered two and this slice adds
three, leaving 36: seven excluded runtime fixtures; two PipeWire
connect-start/teardown-wait candidates needing a real linked GLAD/GIO fixture;
and 27 other SDK/plugin fragments, including remaining V4L2, FreeType font
fixtures, Wayland/xkbcommon, Qt, Python and QSV/VPL. The remaining PulseAudio
default-follow/monitor-device/monitor-reset fixtures remain excluded. The
other 40 original graphics/WebSocket/display exclusions remain outside scope.

Full workspace unittest, final-head dual-core Tier 2/export parity,
Windows/macOS/BSD execution and application/profile/device/display/daemon
behavior are unverified locally. No PR, auto-merge or hosted CI is opened;
the manager owns the combined rollup gate. No target pruning or soldr gc.
