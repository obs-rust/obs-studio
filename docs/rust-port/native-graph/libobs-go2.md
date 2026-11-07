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
on those platforms. GCC and Clang CMake configure/build/CTest each pass all
three executables (11 unchanged C cases) using installed OBS 32.1.2.

Replay sources, original fragments, dependency inputs and inventories are in
`/tmp/b2-libobs-go2/`. CMake evidence: `local-test-k92OvU.log`; five-OS
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
