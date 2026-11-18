# Windows Rust test artifacts (slices 1, 3 and 4)

`ci/windows_test_artifact.py` prepares Windows tests on Linux. Workflow activation,
native Windows qualification and the Qt contracts remain separate work:

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
CARGO_BIN_EXE paths), execute MSVC doctests and native histories, and prepare the
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
