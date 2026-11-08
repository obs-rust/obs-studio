# Batch 4: headless Wayland SDK contracts

Refs #1390. Branch-only pool slice based on `test/1390-b4-sdk2`
(`6fdfbe308d8bf61ad634d062ff66e463d20cd7a0`).

| Unchanged executable | Cases | Original guard | Timeout |
| --- | ---: | --- | --- |
| `test_wayland_keymap` | 5 | Linux/FreeBSD, Wayland enabled, client + xkbcommon | unset |
| `test_wayland_volume_keys` | 1 | Linux/FreeBSD, Wayland enabled, client + xkbcommon | unset |
| `test_wayland_multiseat` | 6 | Linux/FreeBSD, Wayland enabled, client/server + xkbcommon | unset |

The graph adds three targets/tests: 515 → 518 in each map. Every pre-existing
record stays unchanged. Preserve the full original source lists, including
keymap's inclusion of the main Wayland implementation from inside the unchanged
C test, the `obs=wayland_test_obs` definition, libobs/caption/cmocka linkage,
and real Wayland/xkbcommon imported targets. The common SDK helper discovers
optional packages quietly only inside the original host and feature guards.
Missing SDKs select nothing. No new stubs, C-test/public-header changes,
WebSocket edits, workflows or Rust test executables.

The production Cargo compatibility builder has no Wayland feature option;
its new facts therefore default false. CMake and the explicit SDK runtime
contract enable the fixtures. This slice does not claim to add Wayland to
the production Cargo core build.

## Evidence

- RED `d66726e6d`: registration assertion fails on the absent graph entry
  (`local-test-MsHSE5.log`). An earlier harness doc-comment placement error
  was corrected before the RED commit.
- GCC: registration, guard selection, SDK-absent configure and runtime
  contracts pass; all 12 unchanged cases pass through Cargo/cc::Build and
  again through CMake/CTest (`local-test-lz2lIB.log`).
- Clang: same freshly built Application harness, `CC=clang CXX=clang++`,
  passes all four contracts and all 12 cases through each consumer
  (`local-test-8DAyaq.log`).
- CMake inventories: enabled = 3, FreeBSD selection = 3, non-Linux/BSD = 0,
  feature off = 0, Wayland unavailable = 0, xkbcommon unavailable = 0.
  Enabled inventory checks direct commands and unset timeouts. Package-free
  configure covers Linux, FreeBSD, OpenBSD, Windows and macOS selections.
- Whole-graph legacy replacement contract passes (`local-test-ZAXXY2.log`).
- Nine focused Python graph/portability/compatibility/budget modules pass
  (`local-test-lAwB4B.log`, `local-test-MqAg9h.log`). Explicit gersemi 0.25.0
  checks all four changed CMake files (`local-test-w6fNXF.log`).
- Scoped all-target Clippy, `libobs-integration`, `-D warnings`, passes
  on Linux (`local-test-ocYtyo.log`) and `x86_64-pc-windows-msvc`
  (`local-test-c0oO81.log`). Windows is compilation/lint evidence only.
- One primary clud-review reviewer reports clean; independently confirms
  exactly three additions and every pre-existing record unchanged.
- Managed rustfmt of all changed Rust files passes (`local-test-82KeeF.log`).
  An earlier workspace `soldr cargo fmt` stalled and was interrupted; no
  cache/daemon workaround. A redundant workspace formatting check was
  refused by clud's verification-ratio guard and was not repeated.

Logs are retained under
`/home/niteris/.clud/tmp/tool-output/local-test-private/`.
Installed OBS 32.1.2 supplies the real shared core for local smoke evidence;
this is not final-head hybrid/C-reference parity. Tests use their original
self-contained keymap/fake-compositor fixtures, no running desktop display.
The caption archive is built from the real repository sources and shared by
the two consumers in the runtime contract.

Test cost, 25 reduced-debug runs each: registration longest 0.04095 s;
guards 0.03947 s; SDK-absent configure 0.23336 s; peak child RSS 19,364 KiB.
Fast contracts own no threads; the multiseat C fixture starts a compositor
child process. Real compiler/CMake subprocess replay retains the existing
native-build budget exception. Measurements:
`build_ci_audit/b4-sdk3/test-cost.json`.

Full `SOLDR_PORCELAIN=0 SOLDR_RUSTC_WRAPPER=none ./lint` ran before push
(`local-test-Dc6vVX.log`). Formatting, gersemi, Python guards, workspace
Clippy and every Clippy feature-set entry passed. Exit 1 is exclusively
Soldr refusing to build the Dylint driver under clud: no deployed driver
for nightly-2026-05-28. Final log lines were inspected. Dylint remains
unverified locally; no toolchain workaround or attestation stamp. The
manager's hosted rollup Fast Gate must cover it.

## Deferred scope

The original 41 SDK/owner fragments now have 12 registrations across the
base and these batch-4 slices, leaving 29 deferred: seven excluded runtime
fixtures, two PipeWire linked GLAD/GIO candidates, one V4L2 descriptor test
requiring final-head core behavior, and 19 other unconverted SDK/plugin
fragments (remaining V4L2, font fixtures, Wayland hotkeys/portal, Qt, Python,
QSV/VPL and VST). This is a focused slice, not an exhaustion claim. The 40
original graphics/WebSocket/display exclusions remain outside scope.

Full workspace unittest, final-head dual-core Tier 2/export parity,
application/profile/protocol/device/desktop behavior and native
Windows/macOS/BSD execution are unverified. No PR, auto-merge or hosted CI:
the manager owns the rollup gate. No target pruning or soldr gc.
