# Batch 2 continuation: image version and OSS parser

Refs #1390. Extends `test/1390-b2-libobs-go`, retaining its five registrations.
This slice adds three unchanged executables, not the entire g–o inventory.

| Executable | Cases | Selection | Timeout |
| --- | ---: | --- | --- |
| `test_image_file_version` | 5 | not Windows | none |
| `test_image_file_version_filetime` | 4 | all hosts | none |
| `test_oss_sndstat` | 2 | Linux | none |

All retain real `OBS::libobs` and cmocka linkage, direct invocation and no
skip code or runtime wrapper. No C tests or public headers changed. These
checks need no graphics context, display, hardware or audio service.

The image tests preserve the original per-OS host source attachment from
`test_image_source_host.cmake`: macOS and POSIX unity groups, or the Windows
host implementation. The Windows FILETIME implementation remains conditional.
The shared graph declares those sources explicitly so Cargo can compile them;
the deferred CMake function continues attaching hosts only to the two legacy
slideshow tests, preventing double attachment. The image tests declare Windows
`OBS::w32-pthreads` because their unchanged shared PCH includes threading.h.

RED `227e7fba5` fails with “canonical graph omits test_image_file_version”.
Registration checks cover all five OS selections, exact source lists and
real linkage, absent timeouts/wrappers and removal of legacy declarations.
Original/canonical CMake inventories match for Linux (three), Windows (one),
macOS, FreeBSD and OpenBSD (two each). This simulates selection, not runtime
on those platforms. GCC and Clang Cargo graph compilation/execution and CMake
configure/build/CTest each pass all
three executables (11 unchanged C cases) using installed OBS 32.1.2.

Replay sources, original fragments, dependency inputs and inventories are in
`/tmp/b2-libobs-go2/`. Cargo/CMake evidence: `local-test-cnq704.log` (all exit codes zero; expected
missing-sndstat diagnostic is asserted by the unchanged test); five-OS
selection: `local-test-WjIwmg.log`; focused Python guards:
`local-test-HzbNkK.log`. These external-library smoke checks do not establish
current-head hybrid/C-reference runtime or exact dynamic export parity.
Native Windows/macOS runtime, full application/profile/protocol validation
and broader #1390 parity remain unverified. Existing Rust unit/oracle
coverage is unchanged; Tier 4 is inapplicable to this inventory change.

Other g–o tests remain as listed in `libobs-go.md`, except the image-version
and OSS parser tests completed here: image decode/EXIF and remaining graphics
checks need separate decoder composition; image-size, GL, hotkeys, Opus and
several output tests need graphics/display/plugin integration; media tests
need FFmpeg fixtures; inhibit checks need OS services; connector, JACK, Lua,
output-host, startup and Rust-static checks remain separate dependency slices.
No PR, auto-merge or hosted CI is requested for this branch-only handoff.

Initial GREEN registration passed (`local-test-s5eESI.log`); 25 reduced debug
runs of that compiled check took at most 0.042 s, peak RSS 10,888 KiB, with no
test-owned threads (`local-test-RMILUu.log`). The later exact-source assertions
in the registration module remain uncompiled locally: the subsequent Soldr
compile stalled, and scoped Clippy was refused by Soldr's active build-lock
preflight (`local-test-8k7sSc.log`). No lock was removed or guard bypassed.
The native replay compiled successfully with the documented
`SOLDR_RUSTC_WRAPPER=none` setting and confirmed each artifact's selected real
libobs resolution with `ldd`. Whole-graph legacy replacement passes
(`local-test-jg5pRy.log`); scoped rustfmt and `git diff --check` pass.
One primary read-only clud-review found no issues. Full lint/unittest were
not run; the unittest wrapper's pruning is prohibited for this assignment.
The manager must compile/run the enhanced registration and scoped Clippy
on the rollup before claiming those checks pass.
