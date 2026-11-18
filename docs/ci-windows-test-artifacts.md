# Windows Rust executable artifacts (first slice)

`ci/windows_test_artifact.py` prepares a **partial** Windows bundle on Linux:

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

The manifest declares `coverage: rust-test-executables`. It cannot claim
`doctests_complete` or native fixture completeness. This slice has build and
verify commands only. No workflow job, check name, scope or gate is changed.

Before replacing the Windows Rust job, follow-up slices must prove the common
Linux/Windows source path contract (including embedded CARGO_MANIFEST_DIR and
CARGO_BIN_EXE paths), capture and execute MSVC doctests, extend native fixture
prepare/replay for Windows RC/PCH and compiler discovery, and prepare the
published Qt SDK contracts and runtime DLL closure. Existing Rust categories,
all native assertions and unchanged C compatibility tests remain required.
Then enrich the existing Linux Windows-link producer rather than cross-building
twice; require its successful artifact in CI Gate and measure native execution
and transfer costs before claiming a speedup. Windows Tier 2 is separate work.
