# macOS Tier 2 packet and optional hosted proof

Refs #5009. Linux builds every input; macOS executes the packet. The existing
required `Rust Tier 2 (macos-26)` job remains the native baseline until the
activation gates below pass. The optional proof grants no required-check credit.

`ci/macos_tier2_produce.py` prepares both CMake cores, the complete unchanged
CTest aggregate, independent Cargo-native cores/renderers/capture modules/probes,
the stale-manifest fixture and the existing Application-category native contract.
It invalidates old completeness first and records the clean source SHA and actual
image hashes. CMake's independent configure provenance must name that same SHA.

Both preparation and execution use the common **real** checkout root
`/opt/obs-tier2-run-only/src`. A symlink into a slot is insufficient: Cargo embeds
manifest-directory constants that `--remap-path-prefix` does not rewrite.
Use the existing `ci/macos_test_workspace.py place/restore` mechanism for the
producer, retaining the original checkout symlink for Actions post steps. Restore
in an `always()` step; its existing noninteractive sudo retry removes only an
empty common parent under root-owned `/opt`. Never copy a new checkout or patch
strings inside a linked image.

After placement, from the common checkout:

```sh
# Until the required soldr release exists, only the authorized local proof uses:
export OBS_TIER2_SOLDR=/path/to/approved/source-built/soldr
uv run --python 3.14 --no-project ci/macos_tier2_produce.py
OBS_TIER2_CONTRACT=$(uv run --python 3.14 --no-project python -c 'import json; print(json.load(open(".tier2/production.json"))["contract"])')
uv run --python 3.14 --no-project ci/macos_tier2_packet.py stage \
  --output build_ci_audit/macos-tier2-packet \
  --darwin-receipt .tier2/toolchain/darwin.json \
  --contract "/opt/obs-tier2-run-only/src/${OBS_TIER2_CONTRACT}" \
  --forbidden-root /original/producer/slot \
  --archive build_ci_audit/macos-tier2.tar.zst
```

The `--contract` path must be absolute; read the source-relative receipt entry and
prefix the common checkout root when invoking the command. Output directories and
archives must be absent. Use ledgered scratch directories for disposable evidence.
The producer needs the normal Linux CMake/Ninja/Qt generator dependencies and
authenticated dependency downloads; it never starts a hosted macOS build.

The schema-2 manifest seals all files, executable permissions and directories,
including intentionally empty working directories. Every signed arm64 image uses
the shared CodeDirectory page verifier. The native preparation receipt must be
complete, and the production receipt must cover every selected CMake/native root
and the contract executable before `native_contract_prebuilt=true` is written.
Source resources, framework aliases, Qt release plugins and their recursive load
closure travel with the packet. Native property parity remains an independent
qualification gate, not a claim inferred from a cross registry count.

The packer scans every copied file, including Mach-O and UTF-16 resources, for the
original slot roots and producer home. C/C++/Swift/Rust diagnostic prefix maps
remove toolchain-home strings during compilation. The prepared stale-manifest
fixture first passes its original assertion-only check on Linux. Only UTF-8
producer diagnostics are then normalized from the producer home to `/toolchain`;
the normalization receipt retains original and portable hashes, and every
prepared-file existence/hash check remains in the runtime CMake receipt. Binary
evidence containing a producer path is rejected and must be rebuilt. No signed
runtime is edited during transport.

Compiler-rt's signed ASan/UBSan dylibs retain their Conan install names. The
packet stages them alongside the dependency dylibs and declares
`DYLD_LIBRARY_PATH`; hosted proof verifies actual loading of the staged path and
checks missing-environment and missing-runtime failures before all four suites.
There is no install-name rewrite, so Linux re-signing is unnecessary.

The consumer verifies the externally supplied archive SHA256 before extraction,
then the sealed file/directory/image inventory before execution. It rejects
unsafe names, archive links, collisions, extra files and a mismatched source SHA.
It restores the common root from the manifest rather than constructing a new
producer-attempt path on reruns. The Actions checkout stays at its original root.

CTest transport retains every command, argument and property. Only known
PATH-resolved tool **executable slots** receive logical identities; nested
`cmake -E env` executable slots are handled too. Literal path arguments, unknown
same-basename programs, environment and working directories stay exact. Bracket
serialization preserves empty arguments, backslashes, `::` names and list values.
The consumer round-trips the entire registry through real CTest before running.

## Maintainer proof trigger

Upload exactly `macos-tier2.tar.zst` and `macos-tier2.tar.zst.sha256` as one Actions
artifact from the **same clean source SHA**. Retain the upload's immutable artifact
ID and the packer's archive digest. A mutable artifact name is never an input.
Once this workflow is available on the default branch, a maintainer may invoke:

```sh
gh workflow run build-project.yaml --ref CERTIFIED_SOURCE_REF \
  -f macos-tier2-proof-artifact-id=IMMUTABLE_ID \
  -f macos-tier2-proof-sha256=ARCHIVE_SHA256
```

The optional `macOS Tier 2 run-only proof (optional)` job in the existing workflow
is off when the artifact ID is empty. A proof dispatch skips ordinary build jobs.
The consumer downloads by ID, verifies the artifact's source SHA, installs only
prebuilt Python, poisons compiler/tool discovery, then runs the shared prepared
validator. It performs the two CMake C/hybrid passes, the exact
`native_libobs::macos_core_builds_from_the_canonical_manifest` selector, both
Cargo-native passes, export/framework-byte checks, render/capture/data/registration
checks and every wrong-module 14 / wrong-core 12 negative control. Nonzero exits
remain failures. No soldr, dependency cook, compiler, Swift or dsymutil preparation
runs on macOS. No dispatch is part of local qualification.

## Activation checklist

1. Release soldr containing forge-ci/soldr `efdec63674b01b45553212c01e0422e749f2c289`
   / #4168, then consistently bump repository setup/cross-tool pins. The minimum
   numeric version is **the first release containing that commit**, not the
   source binary's reused `0.9.53` version string. Remove the scratch override.
2. Produce a fresh packet on the final clean commit using that released pin.
   Require zero source/build/home path leaks and publish archive compressed size,
   SHA256, sealed manifest, production receipt and tool identity. Rebase or source
   changes invalidate prior evidence.
3. Independently configure the native registry at that same commit and compare
   all names/commands/properties using `ci/macos_tier2_registry.py`; do not infer
   parity from historical names or the transport round-trip.
4. Run the optional hosted proof by immutable ID. Retain all four entire CTest
   results, exact native contract output, sanitizer loader/negative-control output
   and timings. A missing compiler or failed assertion must never trigger a build
   fallback or a successful skip. Exercise a consumer-only rerun with the original
   producer artifact and common-root receipt.
5. Only after those gates pass, switch the required job to the Linux producer plus
   run-only consumer while preserving its check name, scope and fail-closed gate.
   The queue must still require actual selected execution. Compare complete
   producer/transfer/run cost with the native baseline before claiming savings.

This branch's sealing/consumer self-tests are transport evidence. They do not
certify the retained pre-portability archive or replace a fresh common-root build
and hosted proof.
