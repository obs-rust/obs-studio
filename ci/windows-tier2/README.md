# Windows Tier 2: Linux producer, hosted execution

This branch is an incomplete qualification prototype. The required Windows job
still builds and runs its original four passes. There is no native compilation
fallback in the prototype. The explicit hosted exception supplies release VC++
runtime DLLs from System32; their AMD64 headers and recursive imports are checked.

`native-test-names.json` preserves all 623 names from hosted job 134577207879 at
7e68243cfbe072830a5381a60010020a048ad026. It is independent evidence of names,
not a same-commit CTest command/property baseline. The inventory also reports
subsequent tests from current master. Never call equal counts full-suite qualification.

## Producer

Use the canonical cmocka directory, including its legacy `tests.d` registrations,
optional owner guards, sanitizer policy, and deferred real link support:

```sh
SOLDR_RUSTC_WRAPPER=none SOLDR_PORCELAIN=0 cmake -S cmake/cross \
  -B build_windows_tier2 -G Ninja -DOBS_CROSS_TARGET=win-x64 \
  -DOBS_CROSS_WINDOWS_TIER2=ON -DOBS_CROSS_WINDOWS_TIER2_FULL=ON
SOLDR_RUSTC_WRAPPER=none SOLDR_PORCELAIN=0 cmake --build build_windows_tier2 \
  --target tier2-cross-full --parallel 6 -- -k 0
uv run --python 3.14 --no-project ci/windows_tier2_inventory.py \
  --build build_windows_tier2 --output windows-tier2-inventory.json
```

The inventory command fails for missing native names or unbuilt tests. It always
records `complete=false`: the historical log cannot establish current native
property parity. Missing owners are reported, never replaced by dummy targets.
Qt generators run on Linux; target Qt DLLs and imports remain Windows artifacts.
The real outputs, filters, image-source, transitions, FreeType and win-capture
owners use their canonical CMake definitions and optional test guards. FreeType
comes from the Windows obs-deps package. Capture also builds the x64 hook,
offsets/injection helpers and WinRT module. Owner resources are copied using
the native resource helper; no empty target satisfies an owner guard.

The cross-only driver layer translates native MSVC option spellings for GNU-style
Clang targeting MSVC. Native MSVC/clang-cl options are unchanged. The MSVC ABI
still selects the original per-test sanitizer policy. SDK link-case aliases stay
inside this producer; soldr's existing VFS handles case-insensitive headers.
The source-seam archive-member isolation override is scoped to the producer's
soldr invocation; native Cargo profiles are unchanged.

Linux CTest omits four legacy extensionless Windows commands. The inventory
recovers their complete argument vectors from generated CTest records and
resolves only an existing, contained AMD64 PE32+ `.exe`, retaining CTest's
original properties. Every resolution is recorded, and missing executables
remain unbuilt.

For a failed full build, retain its full log and map every unbuilt test to its
failed Ninja dependency edges:

```sh
uv run --python 3.14 --no-project ci/windows_tier2_failures.py \
  --build build_windows_tier2 --log producer.log \
  --inventory windows-tier2-inventory.json --output windows-tier2-blockers.json
```

The compile-only stale-manifest regression can be prepared on Linux without
editing the original fixture. Its single-config nested build uses RelWithDebInfo
(the provisioned xwin CRT is release). The receipt records hashes of both original
CMake inputs, target/profile, exact preparation command, status, source SHA and
time. The hosted assertion checks that receipt and input hashes without compiling:

```sh
uv run --python 3.14 --no-project ci/windows_tier2_fixtures.py \
  --build build_windows_tier2 --source-sha "$(git rev-parse HEAD)" \
  --output stale-manifest-receipt.json
```

The full producer is still blocked. In particular, this branch does not yet
produce the independent Cargo cores, native loader/renderer contract helpers,
bundled obs-sys check, independent same-commit native registry parity, or a complete
schema-2 archive. The required targeted clang-cl frontend is tracked in
[forge-ci/soldr#4167](https://github.com/forge-ci/soldr/issues/4167); current GNU
SDK arguments cannot be used in clang-cl mode. The
older `windows_tier2_probe.py stage` remains a schema-1 four-test diagnostic and
cannot enable hosted execution. No complete compressed size or successful full
producer timing is claimed.

## Consumer contract

`ci/windows_tier2_run.py` downloads nothing itself. The optional job in the
existing `build-project.yaml` downloads one exact artifact ID from one specified
run using read-only permissions. Its `windows-tier2.tar.zst` SHA256 must equal the
separately selected repository variable before any extraction or execution.
Python 3.14 reads zstd; neither soldr, LLVM, a compiler nor vcpkg is installed.

A runnable schema-2 manifest requires:

- `complete=true`, `coverage=native-full`, target `x86_64-pc-windows-msvc` and
  `source_sha` equal to the checked-out workflow SHA.
- `files`: SHA256 of every staged file. Archive extraction rejects traversal,
  links, duplicate case-folded names and special files. Unexpected files fail.
- `cores`: four isolated `obs.dll` paths keyed `cmake-off`, `cmake-on`,
  `cargo-off`, `cargo-on`; `exports` must match the nonempty named COFF exports
  independently parsed from all four transported cores.
- `tests`: complete tokenized CTest records, including every command and property.
  `native_baseline`: independent same-commit clean native configure provenance and
  matching records. The consumer materializes a flat registry and asks real CTest
  to prove the relocated commands/properties match before running it.
- `runtime_directories`, an exact `hosted_runtime` import contract, all PE/data
  files, and `native_contract` containing hashed loader/graphics probes, both
  CMake/Cargo D3D11/OpenGL renderers and the independent Linux bundled obs-sys
  check receipt. Both positive and wrong-core/wrong-renderer controls must pass.
- Optional `fixture_receipts.test_core_module_manifest_source` identifies the
  hashed receipt from Linux preparation. Only this known compile-only fixture can
  be replaced with the checked-in assert-only CMake script. Other compiler/script
  commands fail closed until they have an explicit preparation/assertion split.

Build/source paths use `@OBS_TIER2_BUILD@` and `@OBS_TIER2_SOURCE@` tokens. All
CTest properties survive, including environment, working directories, timeouts,
serial execution, resource locks, skip codes and fixtures. Selected cores lead
PATH; executable/cwd/per-test PATH shadowing fails. Compiler executables are
rejected in payloads, and system runtime directories are the only inherited PATH.
Each of the four whole CTest passes uses `--no-tests=error`, preserves
`OBS_CTEST_JOBS` (4 in the optional hosted job), and propagates nonzero status.
WER original-process dumps and cdb fallback diagnostics are retained.

## Activation checklist and maintainer trigger

1. Resolve every recorded compile/link/owner blocker, preserving optional guards.
   Fix soldr frontend/SDK gaps upstream; there is no Windows-build fallback.
2. Produce actual Cargo reference/hybrid cores and all native contract helpers
   on Linux. Preserve the independent bundled obs-sys check and real version/core
   module identity. Native contract preparation is still outstanding.
3. Obtain same-commit clean native CTest JSON and compare every record/property;
   the 623-name historical snapshot alone is insufficient.
4. Stage every executable, owner, renderer, runtime/data dependency and receipt.
   Audit runtime-read embedded paths, not only harmless debug strings. Emit the
   strict schema-2 archive; the prototype cannot manufacture this evidence.
5. Measure successful full fresh/warm Linux preparation and compressed archive
   size against the 250 MiB target / 500 MiB ceiling. Retain separate compression,
   transport, extraction and Windows execution timings.
6. Upload that archive to a same-repository Actions artifact. Record its immutable
   artifact ID, producer run ID and SHA256. Set these repository variables:
   `WINDOWS_TIER2_RUN_ONLY_ARTIFACT_ID`, `WINDOWS_TIER2_RUN_ONLY_RUN_ID`,
   `WINDOWS_TIER2_RUN_ONLY_SHA256`.
7. Dispatch existing `ci.yml` on the exact source ref used for the artifact:
   `gh workflow run ci.yml --repo obs-rust/obs-studio --ref <source-ref>`.
   This invokes the normal workflow as well. The independent
   `windows-tier2-run-only-probe` job runs only on manual dispatch with all three
   variables present. It defaults off, uses `continue-on-error`, and adds no
   required-gate dependency. Empty/delete the variables to turn it off again.
8. Require all four full hosted passes and every native control with compilers
   absent from the execution environment. Inspect dumps for any runtime failure.
9. Only after qualification, seek a separate change to the required job. Preserve
   scope, required check identity, failure/cancellation propagation and artifact
   retention/rerun behavior. Re-measure four completed queue runs before claiming
   queue latency savings.

No hosted run was dispatched for an incomplete payload.
