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

### CMake-only schema-2 transport

`ci/windows_tier2_pack.py` stages the entire built CMake registry and its PE
dependency closure, generated runtime fixtures, libobs effects, plugin data and
locale trees. It includes both source-relative data and the native Windows
`build/data/libobs` / `build/core/<owner>/data` layout. The empty metadata-module
directory is intentional and remains in the explicit directory inventory.

```sh
uv run --python 3.14 --no-project ci/windows_tier2_pack.py \
  --build build_windows_tier2 --output windows-tier2-cmake \
  --archive windows-tier2-cmake.tar.zst \
  --dll-root .deps/obs-deps-2026-10-25-x64/bin \
  --dll-root .deps/obs-deps-qt6-2026-10-25-x64/bin
```

The full producer compiles runtime path literals relative to the unchanged
`build/test/cmocka` working directory, remaps C/C++ and Rust source diagnostics,
and embeds PDB basenames. Linked PE strings are never shortened or patched:
suffix merging can leave other pointers into them. Staging scans every PE and
every other copied file for the producer source/build/home roots in UTF-8 and
UTF-16; any survivor fails. CTest commands/properties use distinct source/build
tokens and PATH-resolved tool identities. Test names, including `:`, remain
metadata rather than filenames. All filenames and directory entries use `/`.
Qt's release plugin trees and their recursive imports retain the exact
source-relative `.deps` locations used by CTest's PATH/QT_PLUGIN_PATH properties;
debug plugins and PDBs are excluded. win-capture's dynamically located graphics,
offset and injection helpers also enter its module data directories.

The archive is **not runnable as a full suite**: `schema=2`, `complete=false`,
`coverage=cmake-part-only`, and `pending[0]="cargo parts pending"`. The existing
full consumer rejects it before execution. Its remaining markers retain native
same-commit registry qualification and hosted execution. The command runs the
unchanged stale-manifest producer fixture and seals its receipt; it does not
replace that test with compilation on Windows. `verify()` in the packer checks
partial transport only. It cannot grant full-suite admission.

### Kept Windows cross tree and handoff

Reuse `ci/tier2_kept_tree.py` (#4954), with a **separate** Windows namespace and
one fixed absolute build path per runner. Configure the cross tree as
`$WIN_BUILD/tree`; its `.clean` marker, source snapshot/replay, crash recovery,
fsync and locking follow the existing helper. Retain the Rust target inside
that same tree with the producer-only option added here:

```sh
uv run --no-project ci/tier2_kept_tree.py restore \
  --kept "$WIN_KEPT" --build "$WIN_BUILD" --src "$GITHUB_WORKSPACE"
if [ ! -f "$WIN_BUILD/tree/CMakeCache.txt" ]; then
  cmake -S cmake/cross -B "$WIN_BUILD/tree" -G Ninja \
    -DOBS_CROSS_TARGET=win-x64 -DOBS_CROSS_WINDOWS_TIER2=ON \
    -DOBS_CROSS_WINDOWS_TIER2_FULL=ON \
    -DOBS_CROSS_WINDOWS_TIER2_RUST_TARGET_DIR="$WIN_BUILD/tree/rust-target"
fi
cmake --build "$WIN_BUILD/tree" --target tier2-cross-full --parallel 6
# Stage and seal the payload while the tree is present; save only on success.
uv run --no-project ci/tier2_kept_tree.py save \
  --kept "$WIN_KEPT" --build "$WIN_BUILD" --src "$GITHUB_WORKSPACE"
```

Admission must remain restricted to the existing trusted queue/runner path.
Choose a runner-specific `$WIN_KEPT` (for example, an adjacent
`windows-tier2-kept-<configuration-digest>` directory under its existing tmpfs area); never share it
with Linux Tier 2 or another concurrent runner. The pinned toolchain, dependency
archives and source identity still enter normal CMake/Cargo freshness. Keeping
the tree is a cache, never proof of passing tests or native parity. No Dell host
or workflow setting is changed by this prototype.
The configuration digest must cover the soldr/compiler/SDK identity, dependency
pins, build profile and lane options; a mismatch starts a new tree. On a hit,
Ninja handles changed CMake inputs normally after source-mtime replay. Do not
force a configure on every warm run: the measured forced configure below is
substantially more expensive than the warm build.

For a second **Linux job pinned to the same Dell**, publish the sealed archive
and its SHA256 through the existing `handoff-put --root "$WIN_HANDOFF" --key
"$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT" --pack "$WIN_PACK" --labels "$LINUX_LABELS"
--marker "$HOST_MARKER"`; use `handoff-get` and fall back to the same immutable
Actions artifact on a miss. Its `local` output can suppress only that pinned
Linux job's download/upload. **Hosted Windows always needs artifact delivery**;
it cannot read the Dell filesystem, so never suppress its upload based on the
Linux handoff result. Use a Windows-specific handoff namespace, bounded by the
helper's existing age/byte limits. No new network share or Windows host is needed.

Observed CMake-only preparation before relocation was 1,108.020 s with empty
outputs and 22.275 s with the retained warm tree. Hosted Linux without the kept
tree pays fresh preparation plus packing/upload; these local timings are not
hosted measurements. The same-SHA source/dependency/toolchain caches can reduce
that cost, but a warm compiler cache does not imply a retained Ninja tree. At
20 MB/s, artifact upload alone costs compressed bytes / 20,000,000 seconds;
add hosted download and Windows extraction. The 250 MiB target is 13.1 s and
the 500 MiB ceiling is 26.2 s at that rate, before contention/service overhead.
The 18.5-minute fresh producer can exceed the historical 15.3-minute Windows
job median. Warm retention is therefore central to the Dell design; a queue
latency reduction and full Cargo-inclusive cost remain unqualified.

### CMake qualification measurement (2026-11-18)

After rebasing onto master b29f7d7a1, the registry includes the new
`test_graphics_device_queries`: 633 executable tests plus the prepared
compile-only fixture, with none of the earlier 632 executables removed. The
CMake-only archive is about **61.30 MiB**, below the 250 MiB target and 500 MiB
ceiling. It contains 2,180 hashed files, 117 directories and 698 PE image copies
(including native/source plugin-data lookup layouts), without OBJ/PCH/PDB caches.

The local Linux measurement took about 3.19 s to pack at zstd level 3, 2.92 s
to extract, 9.03 s to verify and 1.06 s for the final copied-byte path scan.
All 634 commands and property records round-tripped through real CTest. The
relocated stale-manifest assertion ran successfully without compiling. Both
transported CMake cores retain the same 1,796 named exports. These are Linux
transport/assertion measurements, not Windows extraction or execution evidence.

The relocation rebuild took 113.30 s to configure and 616.35 s to build; its
outputs were rebuilt with new path flags while toolchain/dependency caches were
already present. A subsequent forced configure took 219.12 s and its warm build
18.04 s on the shared host. Neither is a new cold hosted benchmark. At 20 MB/s,
this CMake part alone needs about 3.21 s of upload before contention and service
overhead. Cargo additions will increase the complete artifact and producer cost.

Full same-commit native parity remains unavailable from existing CI: the master
b29f7d7a1 Push run 44306821448 had no jobs/artifacts at inspection. The latest
available Windows Tier 2 job 134805045208 (run 44301710451, source 708ad64e7)
reports four passes of 633 tests, but neither emits nor uploads its CTest JSON.
Compared with that historical name list, the cross registry adds only
`test_graphics_device_queries` and removes nothing. Full native command/property
differences cannot be computed from a name-only log; matching names are not
treated as native qualification. No workflow was dispatched to manufacture it.

## Cargo producer qualification

`ci/windows_tier2_cargo.py` prepares the native Windows validator's Rust release
archive, Cargo-native core pair, renderers and loader/graphics probes on Linux.
It also links the existing Application-category Windows test executable without
running it and performs the independent bundled obs-sys check. The Linux harness
runs the original `native_libobs::windows_tests::core_builds_from_the_canonical_manifest`
assertions with an explicit Windows target; it executes no Windows program.

This requires the real targeted clang-cl API from forge-ci/soldr PR #4169
(Refs #4167). The first release containing that API is not assigned yet; the
source-built binary still reports 0.9.53, so its version string is insufficient.
The local `OBS_TIER2_SOLDR` override selects that executable without changing the
repository's tooling pin or installing tools on hosted Windows:

```sh
OBS_TIER2_SOLDR=/absolute/path/to/source-built/soldr \
  uv run --no-project ci/windows_tier2_cargo.py \
  --build build_windows_tier2 \
  --dependencies .deps/obs-deps-2026-10-25-x64 \
  --output build_ci_audit/windows-tier2-cargo
```

The producer asks that exact executable for its target-qualified compiler,
librarian, resource compiler and SDK. It records the executable's hash, keeps
full per-step logs and status receipts, and stops on a failure or timeout. Only
a complete successful preparation writes `cargo-receipt.json`. A completed
output cannot be reused and mistaken for a new proof. This preparation alone
does not change schema-2 packet completeness or authorize hosted execution;
transport, relocation, native parity and hosted assertions remain separate gates.

Qualification of soldr source `7db1f398a3fb` found two upstream gaps. The native
C-reference link cannot resolve uppercase `MSVCRT.lib` and `OLDNAMES.lib` requests
against the lowercase xwin library files. Header VFS coverage does not provide
linker library-name aliases. A tiny object containing only uppercase defaultlib
directives reproduces this with the targeted driver. The bundled obs-sys build
also observes `CC_x86_64_pc_windows_msvc=clang-cl` after entering the soldr Cargo
front door, replacing the selected targeted driver; its w32-pthreads DLL link
fails to spawn the linker. Both need qualification in soldr#4169. The Rust release
archive and Windows Application-category executable do cross-link, but those
successes do not establish Cargo core or bundled-builder parity.

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
