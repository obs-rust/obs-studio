# Batch 3: recording shutdown waits

Refs #1390. Based on `test/1390-b2-rollup` (#3847), excluding all entries
already registered there. Continues the interrupted pool recording-shutdown
claim with two unchanged headless QtCore tests:

| Test | Original timeout | Cases |
| --- | --- | --- |
| `test_recording_shutdown_inactive` | 15 seconds | 2 |
| `test_recording_shutdown_wait` | 60 seconds | 4 |

Both retain the Linux and optional QtCore guards, frontend widget includes,
real `OBS::libobs`, QtCore and cmocka linkage, and direct executable command.
The frontend headers are watched for rebuilds. Original C++ tests and public
headers remain unchanged. No display, device, daemon or OBS startup is needed.
No Rust test executable is added: registration and opt-in runtime contracts
live in the existing Application category.

The registration contract checks sources, linkage, includes, command, timeouts
and absence of extra definitions, flags, environment, skip codes or locks.
It checks Linux, FreeBSD, OpenBSD, Windows and macOS with QtCore present and
absent. The opt-in runtime contract builds and runs all six unchanged cases
through both Cargo and CMake, checks CTest commands/timeouts, and checks that
disabling QtCore or Linux removes both tests. Installed OBS is a real-library
smoke fixture; its Nix header layout is handled only in that fixture.

RED commit `d404bb908` fails with `canonical graph omits
test_recording_shutdown_inactive`. The saved starting registration test needed
correction because graph source/library types implement Deserialize but not
Serialize; assertions now inspect the canonical JSON values directly.
Its unvalidated command placeholder also needed the graph's `${target}`
syntax; real runtime replay caught the unsupported literal placeholder.

Other p-r/t-z candidates are deferred, not declared exhausted. In particular,
the full recording shutdown test needs graphics; RTMPS upgrade loads the real
services plugin and needs OBS startup/display; Wayland keymap needs its
Wayland/xkbcommon/caption dependency fixture; QSV encode/update need VPL.
Capture, plugin-runtime, display and device suites stay outside this slice's
headless dependency scope. No `test_s` entries are included.

Final-head hybrid/C-reference Tier 2 runs and exact export parity belong to the
manager's combined rollup gate. Windows/macOS native execution and new Tier 4
behavior are not claimed. This branch changes test registration only.

## Validation

GCC and Clang each pass registration plus the real Cargo/CMake runtime
contract, including all six native cases through each consumer and CTest's
QtCore-off/Linux-off inventory checks. Evidence: `local-test-wkh3cc.log`
(GCC) and `local-test-IUKFov.log` (Clang), under clud's
`tool-output/local-test-private/`. RED evidence is `local-test-eEBzGx.log`.
The CMake smoke fixture scopes the installed package's C-only standard flag
to C rather than propagating it into C++.

Focused graph-slice, sorted-map, cmocka compatibility and nextest-budget
self-tests pass (`local-test-7AZxDe.log`); whole-graph legacy replacement
passes (`local-test-cf1Dyo.log`). Scoped Application Clippy with the real-core
feature passes (`local-test-Dn6zIS.log`). Gersemi 0.25.0 checks both changed
CMake fragments, rustfmt and `git diff --check` pass. Full lint/unittest and
hosted CI are not run; no target pruning or garbage collection is performed.

Test cost: 25 reduced debug registration runs, longest 0.124 seconds and
peak child RSS 15,076 KiB; the registration contract has no own threads.
The runtime contract uses
compiler/CMake subprocesses under the existing native-build budget exception;
the unchanged native cases start at most one worker thread at a time.
