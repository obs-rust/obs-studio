# Windows Rust test artifacts (slices 1, 3, 4 and 5)

`ci/windows_test_artifact.py` prepares Windows tests on Linux. Workflow activation,
and native Windows qualification remain separate work:

```sh
uv run --no-project ci/windows_test_artifact.py build --output build_windows_test_slice
uv run --no-project ci/windows_test_artifact.py verify --manifest build_windows_test_slice/manifest.json
```

The build reuses `windows_link_check.link_args()`: soldr cross-links the default
workspace tests for `x86_64-pc-windows-msvc`. `--base <commit>` preserves the
existing changed-crate selection. A successful Cargo JSON stream must contain
exactly the selected test-target inventory before publication.

The collector and checksummed file transport come from #5019's
`macos_test_artifact.py`, with explicit target/verifier arguments and unchanged
Darwin defaults. The bundle includes test executables, Cargo helpers and
build-script output trees. It retains each test's package working directory,
Cargo environment and target-specific OUT_DIR. It does not copy executable
PDBs. The verifier checks the complete file inventory, AMD64 PE32+ console
headers, normal/delay-load DLL imports and recorded VC runtime requirements.
Unknown non-system dependencies and debug CRT dependencies fail preparation.
PE import parsing is shared with `windows_zip_check.py`.

Soldr's xwin SDK supplies MSVC/Windows headers and import libraries. Runtime
prerequisites are separate: the supported Windows consumer must provide the
OS UCRT and x64 VC 14.x redistributable (currently `VCRUNTIME140.dll`), or stage
verified redistributable DLLs beside the executables. `runtime_dlls` records
these prerequisites; it does not claim those DLLs are bundled. PE verification
is not evidence of successful execution on Windows.

The producer declares `coverage: rust-tests-doctests-native-fixtures` after all
selected compilation succeeds. The older `rust-test-executables` format remains
explicitly partial. No workflow job, check name, scope or gate is changed.

Stable cross-doctests reuse #5019's capture and execution code with an explicit
MSVC target/verifier. A small link.exe-syntax adapter resolves soldr's managed
lld-link and xwin library directories: the driver printed by `--print-linker`
speaks GNU syntax, while Rust's MSVC target requires link.exe arguments.
Captured binaries retain `.exe`, package cwd, Cargo environment and harness
arguments. Rustdoc checks `compile_fail` and `no_run` on Linux; its merged 2024
harness retains runnable and `should_panic` assertions for native replay.
Unmergeable standalone `should_panic` cases fail preparation rather than lose
assertions. `run-doctests --manifest PATH --root ROOT` verifies the entire bundle
and uses the shared runner on Windows, retaining its compiled-root rejection.

`windows_native_fixtures.py` reuses the shared Application-category preparation
runner and Rust prepare/replay transport. Its 15 default Windows contracts cover
profile/assertion policies, core/plugin dependencies, archive/executable links,
independent C/C++ flags, explicit sources, graph/CMake consumers, generated
headers, RC headers/definitions/unity and MSVC PCH headers/flags/unity/archive
linking. Every PCH `.obj` and `.pch` observation remains in the original test.
Darwin-only languages and GNU PCH keep their existing guards. Qt and optional
libobs contracts remain outside this inventory.

Discovery and compilation run on Linux against soldr's managed clang-cl mode,
llvm-lib, lld-link, llvm-rc and xwin headers/CRT/import libraries, including its
case overlay. CMake uses Ninja and the DLL CRT; xwin does not provide debug CRT
libraries. Recipes, input hashes (including `.rc`), expected rebuild decisions,
`BUILD_HOST` (Linux), `BUILD_TARGET` (Windows), intermediate snapshots and every
deferred runtime assertion are recorded. PE verification accepts DLL subsystems
only for native fixtures and permits fixture imports only when staged in the
inventory. Debug CRT and unknown imports still fail closed.

`OBS_NATIVE_FIXTURES` selects replay in the original test bodies. Missing,
incomplete, incompatible or corrupt fixtures fail; compiler callbacks cannot
run as fallback. `OBS_NATIVE_FIXTURES_PREPARE` and `OBS_NATIVE_FIXTURE_TARGET`
select producer preparation. Both modes together fail. DLL loads, dependency
loads, fresh child probes, executable stdout/status, CTest and marker-file
assertions execute on the native consumer. `BUILD_HOST` describes compilation,
not the runtime OS.

Measure the complete checksummed compressed archive against the **64 MiB soft
Windows budget**, retaining all fixtures and assertions. Record archive bytes,
packing/unpacking time and separate WAN publication evidence. A budget overrun
needs transport/measurement work before activation, never coverage removal.

Use the shared transport's optional zstd format for the complete Windows payload:

```sh
uv run --no-project --python 3.14 ci/macos_test_workspace.py pack --bundle build_windows_test_slice --archive windows-tests.tar.zst --compression zst
```

Packing and extraction of this format require Python 3.14 or newer. The existing
shared `unpack` command detects the format after verifying its SHA256 sidecar.
Level 8 with a 128 MiB window and long-distance matching shares repeated Rust
code and native PCH snapshots across archive members. Every file, hidden input,
permission and checksum remains present. The shared helper's default is gzip;
no Darwin workflow or default changes.

The hosted proof run must show all of the following before activation:

1. A demonstrated Linux-to-Windows root/drive mapping for embedded
   `CARGO_MANIFEST_DIR`, `CARGO_BIN_EXE`, OUT_DIR, source reads, native recipes,
   CMake/CTest paths and child invocation, retaining the root-mismatch rejection.
2. All eight original categories and three helpers executing natively, with
   unchanged test discovery, cwd, environment, child selectors and assertions.
3. The x64 VC 14.x redistributable and OS UCRT available in the actual loader
   search path; every normal/delay import and staged dependency DLL resolved.
4. Captured runnable/`should_panic` doctests executing successfully; a deliberately
   broken runtime assertion fails replay. Producer logs must also show the
   `compile_fail`/`no_run` checks and their negative controls.
5. All 15 native fixture histories replayed with zero compiler discovery/build
   calls on Windows; every RC/PCH load/run, profile result, dependency reload,
   stdout, CTest and marker assertion succeeds, and every event is consumed.
   Missing/corrupt manifests, snapshots and changed recipes must fail closed.
6. The compressed archive size, upload, download and verification timings,
   producer/consumer dependency timeline and real Windows execution costs.
   These establish feasibility; four comparable hosted runs are still needed
   for a measured speedup claim.

Before replacing the Windows Rust job, follow-up slices must prove the common
Linux/Windows source path contract (including embedded CARGO_MANIFEST_DIR and
CARGO_BIN_EXE paths), execute MSVC doctests and native histories, including the prepared
published Qt SDK contracts and runtime DLL closure. Existing Rust categories,
all native assertions and unchanged C compatibility tests remain required.
Then enrich the existing Linux Windows-link producer rather than cross-building
twice; require its successful artifact in CI Gate and measure native execution
and transfer costs before claiming a speedup. Windows Tier 2 is separate work.

Linux qualification on 2026-11-18 used CI-pinned soldr 0.9.45. The complete
producer passed in 112.60 s: eight categories, three helpers, 55 additional native
PE images, 15 completed histories and 1,356 checksummed files. All images passed
PE/import verification. The manifests record Linux BUILD_HOST and MSVC
BUILD_TARGET; RC has 18 events and MSVC PCH has 22, including deferred runtime
assertions. The current workspace has no runnable doctests; a separate real
four-case probe retained a verified PE harness. Compile-fail/no-run mutations
failed preparation; equivalent Linux execution rejected broken runnable and
should-panic assertions. Native Windows replay remains unverified.

The same complete 299,744,188-byte payload measured 84,238,817 bytes with gzip
(80.34 MiB, 47.53 s packing) and **36,377,903 bytes with zstd** (34.69 MiB,
5.33 s packing, 1.42 s unpacking plus full verification). The zstd archive is
within budget without omitting fixtures. These are local observations, not WAN
or hosted speedup evidence.

The Application category regression also checks that omitted target-specific
contracts return before entering fixture mode. RED `5c3348095` created empty
evidence for an inapplicable Swift contract; GREEN `36d787fd8` moves Swift/GNU
PCH target guards before fixture entry. All eight Rust prepare/replay tests pass. The new
test takes at most 39 ms with 14.5 MiB peak RSS over 20 runs, starts no threads,
and re-executes three child tests on Linux. No category executable was added.


`ci/windows_qt_artifact.py` prepares slice 5, then appends to that same bundle:

```sh
uv run --no-project --python 3.14 ci/windows_qt_artifact.py build --work build_windows_qt --bundle build_windows_test_slice
uv run --no-project ci/windows_test_artifact.py verify --manifest build_windows_test_slice/manifest.json
# Only after the common source root is qualified on native Windows:
uv run --no-project --python 3.14 ci/windows_qt_artifact.py run --manifest manifest.json --root ROOT
```

It uses the exact Windows Rust job's Qt 6.11.1 relwithdebinfo archive, SHA256
`b05c17386d2a1b0564b00755bc1fba152a18eaac34e512d9315922990715b751`,
and the matching Linux Qt archive, SHA256
`f0edfc3432bd57058bbb9a2cf18aacceb55a2564b018346778c34eb0e3369dbf`.
Linux moc/uic/rcc run while the MSVC Qt import libraries and headers target Windows.
The original published-SDK resolver and all four Rust integration contracts run:
metaobjects; remux; product/meter; frontend utilities. Their graph, source inventory,
optional Qt Test/OS guards, generator and CTest-property assertions are retained.
Target executions are recorded with their original arguments, cwd, environment,
loader-path prepends and timeout. They remain PENDING until Windows executes them.

Cmocka 1.1.7 is cross-built as a real DLL and import library from the existing
shared helper's SHA256-pinned source. Function detection uses real MSVC links;
static-library probes would falsely accept unavailable POSIX functions. The
Linux pkg-config executable, version and checksum are recorded in `qt_inputs`;
its search path contains only the cross-built cmocka package. No Windows vcpkg
or pkgconf executable runs on Linux or is required by the run-only consumer.
Native and Qt preparation share soldr's clang-cl/xwin/LLVM discovery and CMake
toolchain, including managed llvm-mt. C++ wrappers enable `/EHsc`. Qt preparation
also defaults to C++17 with accurate `__cplusplus`, matching Linux clang; explicit
per-target standard flags follow the defaults and still take precedence. The other
native histories keep their existing standard defaults. Missing cross tools fail preparation.
Cargo forces Ninja after soldr's managed tool preparation, as #5019 requires;
Windows fixture paths also replace Rust `::` with `--` for legal Windows names.

The shared Qt recorder and bundle appender preserve all payload bytes and checksums.
Windows staging includes QtCore/Gui/Widgets, cmocka, `qminimal.dll`, optional imported
modules and their full recursive normal/delay-import closure. OS/UCRT (including the SDK's Windows ICU C API) and supported
VC redist DLLs remain explicit runner prerequisites; they are not xwin artifacts.
The verifier checks every PE/DLL, hashes, the exact image/import inventory, required
plugin and each executable/DLL's actual staged loader search directories.

Activation checklist additions for slice 2: prove all four Qt contracts' pending
entries on hosted Windows, including the generator resource/UI/metaobject answers,
original cmocka assertions, optional Qt Test selection, minimal plugin loading and
transitive normal/delay imports. Poison/remove compilers, pkgconf and SDK tools on
the consumer; missing/corrupt Qt files or DLLs and nonzero runtime status must fail.
Preserve common-root rejection, embedded helper paths and all slices 3/4 histories.

Slice 6 must enrich the existing Linux Windows-link producer, select Qt using the
existing `qt_sdk` scope, append it before final packing, and require the verified
producer artifact in CI Gate. Native Windows becomes checkout/download/verify/run
only: no native compiler, SDK install, vcpkg, Clippy or doctest compilation fallback.
Keep all-target Windows Clippy/Dylint required on Linux, formatting in its required
lane, immutable artifact IDs/attempt receipts, rerun and failure-artifact behavior,
eight-category/native/doctest/Qt coverage, and the 64 MiB complete compressed budget.
Record publication, extraction, verification and execution timings over four
comparable hosted runs before claiming a speedup. Windows Tier 2 remains separate.

The pinned SDK enables `winsdkicu`: its `icuuc.dll` import is the Windows system
ICU C API, not an omitted third-party payload. See [Microsoft's ICU documentation](https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-).
The hosted proof must verify that OS prerequisite alongside UCRT and the VC redist.


Slice 5 Linux qualification (2026-11-18, CI-pinned soldr 0.9.45): the published-SDK
resolver and all four unchanged contracts passed, with inherited Make deliberately
set at producer entry. All four CMake consumers used Ninja/Release. Windows Qt
Test is absent from this SDK; the existing optional Backtab guard was retained.
There are 48 pending target executions: metaobjects 3, remux 11, product/meter 4,
frontend utilities 30. No Rust category binary or assertion was removed or added.

The full bundle contains 1,757 checksummed files, 65 PE executables, five Qt/cmocka
DLLs and 55 native fixture PE snapshots. All 125 images independently passed
managed LLVM inspection as AMD64 COFF/PE, in addition to the shared PE/import,
checksum and loader-directory verification. Actual compiler probes also passed
both the Qt C++17 default and an explicit C++20 override. Runtime remains pending.

The complete archive is **47,849,892 bytes (45.63 MiB)**; with the retained
312,477-byte four-case doctest probe, **45.93 MiB**, within the 64 MiB budget.
The uncompressed tree is 298,672,446 bytes, including its manifest. Shared zstd
packing took 14.60 s; extraction plus complete verification took 3.70 s. No file,
mutation snapshot or assertion was removed to meet the budget. Archive SHA256:
`c2296f6c6e3571bb28ab33d321acc7ca3ee7935b02a961aa9232897bad083b1e`.

The fresh Rust/doctest/native producer took 26:13.94 (peak RSS 4,693,516 KiB),
and Qt preparation took 12:06.87 (356,344 KiB). Rust rebuilt large category
harnesses with LTO while the shared eight-job broker reported over 100 in-flight
requests. These are local contention measurements with cached SDK inputs, not
hosted timings or evidence of a speedup. Hosted Windows must still qualify OS
UCRT/ICU and the five recorded VC 14.x runtime DLLs before activation.
