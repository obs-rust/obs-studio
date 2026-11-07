# Batch 2: additional archive-only decision tests (#1390)

This continuation is based on `test/1390-b2-rust-static`, retaining its eight
source-step registrations. It adds four unchanged C compatibility executables:

- `test_deinterlace_pacing_steps_rust`
- `test_deinterlace_select_steps_rust`
- `test_scene_audio_window_steps_rust`
- `test_source_color_space_steps_rust`

Each uses its original C file, the real production `libobs_rust-static` archive,
and cmocka, in that order. The audio-window executable retains its conditional
non-MSVC `m` linkage. All four retain their archive-availability guard on the
target and test, `libobs` include path, direct command, and absent timeout,
skip code, wrapper, and sanitizer policy. No libobs core, display, graphics
context, capture hardware, replacement shim, or C implementation is substituted.
Their legacy CMake declarations now refer to the shared graph. Existing C tests
and public headers remain unchanged.

The registration check and real Cargo/CMake runtime contract are modules in the
existing Application category binary. No test executable or manifest is added.
The registration check covers archive absent/present selection on Linux, macOS,
and Windows; it does not claim native runtime execution on the latter two.

RED: `b08e8335f` fails the focused registration check with
`missing Rust-static decision steps` before the graph entries exist.

Remaining candidates are deferred rather than silently simplified: scene crop
needs graphics; other scene/transition/source suites link real libobs or inherit
its headers; async upload and deinterlace drawing inherit libobs/SIMDe/config
headers; effect parser suites require private adapters and some preserve Windows
import-library generation; monitor/CoreAudio suites have additional OS and
sanitizer contracts. They remain eligible for separate slices with those
contracts preserved. Previously registered pacing/conversion/source suites are
not duplicated.

This is scoped archive-only compatibility evidence, not final-head dual-core
Tier 2, dynamic export parity, saved-profile/application/protocol completion,
or a claim that #1390 is finished. The rollup admission retains those gates.
