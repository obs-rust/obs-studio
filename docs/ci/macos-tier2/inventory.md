# macOS Tier 2 cross-build inventory (#5009)

Status: Linux prototype only. `rust-tier2-macos` remains unchanged. Its check,
assertions, four CTest passes and loader controls have not been removed.
Initial inspection used master e12874916 (2026-11-17). Before final validation,
the branch incorporated master fd4412dcb, including the merged #5005 aggregate.
The cross graph is deliberately opt-in (`OBS_CROSS_TIER2=ON`, mac-arm64 only).
It is not yet equivalent to the native graph.

## Existing job

`.github/workflows/build-project.yaml` runs on macos-26, timeout 25 minutes,
with soldr 0.9.45, Xcode CAS restore/save, `brew install cmocka` and
`rust/tools/tier2-validate/validate.sh`. `OBS_CTEST_JOBS=3`. The macos-ci
preset inherits the **Xcode** generator, deployment target 13.0 and warnings
as errors. A toolchain/cache restore is not proof that compilation is avoided.

The validator configures `build_rust`, RelWithDebInfo, unit tests and C reference
ON, hybrid backend, frontend/scripting/browser/websocket OFF. Its roots are
libobs, libobs-c-reference, cmocka-tests, libobs-opengl, libobs-metal,
mac-capture, mac-avcapture and mac-avcapture-legacy. #5005 aggregates exactly
these roots as tier2-native; this prototype does not replace or duplicate that
native aggregate. The existing native validator now consumes it; a future
complete cross producer must cover its full closure before switching the job.

| Compiled product | Inputs and purpose |
| --- | --- |
| Hybrid libobs.framework | Canonical unported `LIBOBS_SOURCES`, MACOS host groups, Rust C adapters, caption, generated version/core-module objects, whole libobs-rust archive. |
| C-reference libobs.framework | Same unported/host objects plus canonical ported C groups; same public install name and exports as hybrid. |
| Rust archives | CMake/Corrosion release libobs-rust and its path crates; separate obs-preflight-ffi C ABI archive; later a fresh target-specific libobs-rust for Cargo-native. Standalone `_rust` fixtures consume the real archive and production private adapters from tests.d. |
| cmocka-tests | Unchanged C/C++ tests and fixture libraries from tests.d plus build/test-graph.json, Qt Core/Widgets test owners when found, FFmpeg/jansson/zlib/SIMDe/uthash interfaces. CTest properties include timeouts, skip codes, environment changes and timing/resource locks. |
| OpenGL | Canonical OPENGL_CORE + OPENGL_MACOS groups, glad, Cocoa/IOSurface/OpenGL. |
| Metal | Canonical METAL_SWIFT source set and Objective-C bridging header; Swift 6, ARC/modules, Metal/MetalKit and SDK Swift overlays. This is not just a C/ObjC dylib. |
| Capture modules | mac-capture C/ObjC ScreenCaptureKit groups (non-ARC), mac-avcapture ObjC ARC/modules and legacy ObjC ARC groups. ScreenCaptureKit is weak-linked; camera frameworks include AVFoundation/CoreMediaIO. All three stage locale data into bundle Resources. |
| Cargo-native contract | Application category `obs-tests-app`, feature libobs-integration, exact `native_libobs::macos_core_builds_from_the_canonical_manifest`. Running it builds two cores, both renderers, three capture modules and C loader probes, checks exports/framework byte identity, and writes graphics/capture manifests. |
| Bundled obs-sys | `soldr cargo check -p obs-sys --features bundled` with the explicit Rust archive and native deps; this may invoke build.rs native work and is preparation work, not a run-host step. |

Core framework dependencies: AppKit, AudioToolbox, AudioUnit, Carbon, Cocoa,
CoreAudio, IOKit, transitively CoreGraphics/CoreServices/CoreFoundation/Foundation,
libobjc and libSystem. FFmpeg loads the pinned obs-deps FFmpeg dylibs and an
11-library non-system closure including x264, srt, rist and MbedTLS. The tests
add Foundation/Security, Qt frameworks and fixture dylibs. All must be inventoried
from actual Mach-O load commands, never from Linux `ldd` or host pkg-config.

## Execution retained by the native validator

1. Diff `nm -gU` exports of CMake C/hybrid; fail on any mismatch.
2. Set DYLD_FRAMEWORK_PATH to CMake C reference (OFF), DYLD_LIBRARY_PATH to
   obs-deps. Run test_bitstream with DYLD_PRINT_LIBRARIES and require the selected
   libobs path. Run the complete CTest registry.
3. Repeat loader proof and the complete registry against CMake hybrid (ON).
4. Build/run the Cargo-native core contract and bundled obs-sys check above.
5. For Cargo-native C reference and hybrid separately: compare dylib bytes to both
   framework paths; prove the selected core loads; run each native/CMake renderer
   through the identity probe; require negative controls 14 (wrong renderer) and
   12 (wrong core). Do the same for all three native/CMake capture modules, diff
   their locale data, retain registration checks and negative controls 14/12.
   Run the complete unchanged CTest registry against each Cargo-native core.

Thus there are **four full passes**, all using the same registry, plus the
native contract and loader checks. Optional `--repeat N --tests REGEX` retains
its OFF/ON repeated CTest runs; lldb crash replays remain diagnostic work.
The supplied successful hosted logs contained 655/656 tests per pass. Their
configure+build intervals were 553.04/580.89 s, CTest totals 232.18/223.93 s and
native contract 111.92/113.82 s. These are measurements, not cross-build savings.

## Linux prototype and relocation contract

```sh
uv run --no-project ci/macos_tier2_toolchain.py \
  --output build_macos_tier2_tools --configure build_macos_tier2_cross
cmake --build build_macos_tier2_cross --target tier2-cross -j 4
```

soldr's printer-selected LLVM/ld64.lld and blessed SDK 14.5 produced a signed
arm64 hybrid dylib in the first real link. The opt-in graph adds both development
frameworks, pinned cmocka 1.1.7 built on Linux, the existing test registry and
`--target aarch64-apple-darwin` for its preflight archive. Cargo is consulted on
every archive build so source changes cannot reuse a stale archive.
`tier2-runtime.txt` records actual executable/shared/module targets.

`ci/macos_tier2_bundle.py stage` copies that explicit inventory, validates arm64
ad-hoc CodeDirectory pages through #5008's verifier and hashes every copied image.
Universal dependencies may be thinned by selecting the original arm64 bytes;
no signed load command is edited. `--deps DIR` adds dependency dylibs. The helper
stores CTest templates and rewrites the build root when preparing on the run
host, keeping all CTest properties and resolving CMAKE_COMMAND on the run host.
Staged bytes are signature-verified again before their hashes are published.
Build/framework RPATHs use @loader_path and
install names use @rpath. Source paths remain a **common absolute source-root
contract** because C fixtures compile paths into binaries. An env var cannot
relocate those strings. Use the workspace helper's real `/opt/obs-ci-<run>-<attempt>/src` root
on both hosts before the production cross-build, with the checkout available
there. A normal Linux /home checkout cannot simply become a macOS /Users checkout.
Build paths can also be compiled into fixture images. The manifest records every
image containing its producer build root and refuses relocation unless that
common build path resolves to the staged tree (for example, a symlink within the
common checkout). Rewriting only CTest paths does not fix these binary strings.

The stage helper takes `--verifier PATH` to #5008's `ci/macos_test_artifact.py`
(dependency commit 1d8822d66ee3b38d0754e8da843d4efb03836a4f, merged into
this branch from ci/macos-run-only-rust-tests).
It reuses `verify_directory`; it does not copy its page-hash implementation.
Its envelope reader additionally accepts MH_DYLIB/MH_BUNDLE and zero alignment
padding found in the blessed dependency signatures. The helper is an explicit
prerequisite, not silently fetched at runtime. No changes to the sibling branch.
All 11 signed arm64 dylibs in the libobs non-system closure passed its page-hash
verifier after extraction. Zero alignment padding in a signature superblob is
retained and checked; it is not an unsigned or corrupt dependency.

A prototype manifest always has `native_contract_prebuilt: false`; its runner
refuses to launch it. This prevents accidental compiler execution on macOS.
Only after the blockers below are resolved can the producer attest a complete
bundle and enable the run-only job. No full CTest pass or native contract has
been demonstrated on macOS from this prototype.

## Preparation split and remaining blockers

The follow-up configures the canonical `plugins/CMakeLists.txt` dispatcher from
its real checkout root. The plugin recipes create their real targets and shared
owners, including RTMP/AMF/output, happy-eyeballs, media-playback, image/slideshow,
LUT/filter and text-file. `tier2-modules.txt` records `OBS_MODULES_ENABLED`; the
same list generates the linked core manifest. The observed selection has 23
modules, including outputs and transitions. No test-only list is substituted.

`test_core_module_manifest_source` keeps its registry entry. Linux builds
`tier2-cross-manifest-fixture` with the soldr toolchain. Its receipt preserves
successful configure/build results, the poisoned source-tree file, generated
binary-tree source, build graph and product hashes. On macOS set
`OBS_TIER2_FIXTURE_PREPARED=1`: the same script checks those hashes and recipe
identity without configuring or compiling. The bundle helper transports the
receipt and products and signature-verifies final Mach-O runtime images.

For the exact `native_libobs::macos_core_builds_from_the_canonical_manifest`
selector, `OBS_BUILD_MACOS_PREPARE_ONLY=1` builds on Linux using an explicit Darwin
target, dependencies, Rust archive, soldr compiler/archiver and SDK. Set
`OBS_BUILD_MACOS_MODULES` to the CMake-produced list. `macos-prepared.json` records
each real core/module/probe path and independent failed preparations in `waiting`.
It remains incomplete until both cores, both renderers, all three capture modules
and both probes exist. Neither a failed artifact nor a partial renderer set is a
successful full contract.

On macOS `OBS_BUILD_MACOS_ASSERT_ONLY=1` reads that complete receipt and never calls
a builder. It requires every CMake reference and retains export, framework-byte,
module-kind, data and required-symbol assertions. `macos-run-prepared.sh` is the
explicit prototype consumer: CMake export/loader checks and two complete CTest
passes, the prebuilt exact Rust contract, then the two Cargo-core CTest passes.
The ordinary validator and prototype share `macos-assertions.sh`, retaining both
native/CMake renderer and capture loads, data diffs, source registration and all
wrong-core (12) / wrong-module (14) controls. This is not workflow activation.

The registry now contains all 30 previously missing names. The five additions
relative to the supplied older native list are `test_encoder_group_lifecycle_api`,
`test_encoder_set_group`, `test_hotkey_pair_register`, `test_scene_group_enum_items`
and `test_source_init_context_rust`: registrations from newer source history.
The real x264 owner is configured too. Its helper test is registered before
`enable_testing()` in the native graph, so the cross graph preserves that ordering
and does not add an extra CTest entry. Counts from different revisions are not
property parity evidence.

Registry evidence must come from Linux configuration of both graphs at the same
final commit. No hosted macOS configure/build step may produce the comparison
snapshot. The cross producer includes the provenance hook automatically. Run:

```sh
uv run --no-project ci/macos_tier2_registry.py snapshot --source . --build BUILD --output registry.json
uv run --no-project ci/macos_tier2_registry.py compare --native native.json --cross cross.json
```

The snapshot requires clean tracked sources and matching configure-time commit
provenance. Comparison checks names, commands and every CTest property, replacing
only checkout/build path roots and ignoring numeric CMake backtrace IDs. A count
match, stale build tree or snapshot from another commit cannot pass.

| Blocker | Status / remaining evidence |
| --- | --- |
| 3. CTest runtime compiler | Split implemented; real Darwin fixture prepares on Linux and its assertion phase succeeds with a nonexistent compiler. Registry entry and original source-selection assertion retained. |
| 4. Native runtime compiler | Separate preparation/complete receipts and assertion-only consumer implemented; shared loader/data/registration controls retained. Full preparation/execution waits on the toolchain rows below. |
| 5. Missing owners and registry parity | Real canonical owners configured; all original 30 missing names present. Same-commit name/property comparator implemented. Independent native snapshot/property comparison still required before activation. |
| 6. Empty core-module list | Replaced by the actual enabled canonical plugin graph (23 observed names); both CMake cores use its generated manifest. |
| Darwin compiler-rt | The managed closure supplies all link inputs, including `__isPlatformVersionAtLeast`. Current ASan/UBSan dylibs retain absolute Conan install names. Stage their original signed bytes in `deps` and explicitly declare the consumer's `DYLD_LIBRARY_PATH`; the closure audit records this requirement. Actual sanitizer loading still needs hosted proof. |
| Swift/Metal and SDK 26.5 | The same managed receipt supplies Linux Swift 6.3.2, Darwin overlays, SDK 26.5, LLVM linker and dsymutil. Both CMake and Cargo-native preparation use it. |
| Full staged closure and macOS execution | Still requires all signed frameworks/dependencies, resources and generated data at the common paths, and successful execution of all four full passes plus contracts on macOS. |

## Observed prototype results

### Unreleased managed Darwin interface

CI still pins soldr 0.9.45. The minimum usable release is the first release
containing forge-ci/soldr commit `efdec63674b01b45553212c01e0422e749f2c289`
(#4168). Published 0.9.53 does not contain this interface; the source build at
that commit also reports 0.9.53, so its version string alone is insufficient.
Do not activate this branch until that release is pinned consistently.

For a local proof only, set `OBS_TIER2_SOLDR=/absolute/path/to/source-built/soldr`.
The producer requests `toolchain darwin --target aarch64-apple-darwin
--sdk-version 26.5 --json` and retains the receipt. This override does not alter
CI's pin. `environment.json` contains only explicit build overrides for the
targeted C/C++ drivers, archiver, Swift and SDK, plus dsymutil. The Swift wrapper
preserves the receipt's resource/importer/linker flags. CMake includes soldr's
own toolchain and targets Objective-C with the same compiler-rt resource root.
Sanitizers remain enabled by their existing test recipes.

Activation requires the soldr pin bump, an independently configured same-commit
registry/property comparison, a complete signed runtime/data/dependency packet,
and hosted execution of all four full suites plus the exact native contract and
every negative control. Hosted macOS only verifies and runs; all compilation,
including Swift/Metal, debug symbols, probes and sanitized fixtures, is Linux work.

The source-built API at the commit above was exercised against compiler-rt unit
`bcf66e74b80500ed606891f12996b0af6c8a2f52d32237dd8e3ddc50c7b5db2e`.
Both `libclang_rt.asan_osx_dynamic.dylib` and
`libclang_rt.ubsan_osx_dynamic.dylib` have signed arm64 slices, but their
`LC_ID_DYLIB` is `/work/.conan-home/p/b/compid4110e650ac77/b/rt-build/lib/darwin/`
followed by the filename. The independent sanitized availability probe inherits
that absolute `LC_LOAD_DYLIB`. The strict audit rejects it without an explicit
runtime override. Apple's [dyld reference](https://raw.githubusercontent.com/apple-oss-distributions/dyld/main/doc/man/man1/dyld.1)
specifies that `DYLD_LIBRARY_PATH` searches dylib leaf names before install paths.
The existing consumer already sets this variable to its staged `deps` directory.
`stage --darwin-receipt DARWIN_JSON` adds the original signed ASan/UBSan runtimes;
`macos_tier2_audit.py --library-path DIR` models only that explicitly declared
override and writes a checksum-bound environment receipt. Without it, producer
paths remain rejected. No toolchain files or signed images are rewritten. Hosted
sanitizer execution and loader logs must prove that the packet's copies load,
with missing-runtime and missing-environment negative controls before activation.

The aggregate builds the same native Tier 2 roots, including every cmocka test
and helper. Configuring canonical owners does not imply building unrelated app
plugins. Bundle inventory follows Ninja's actual transitive graph. All Qt host
executable configurations, including RelWithDebInfo and the automoc parser, bind
to the pinned Linux SDK and require ELF headers; Darwin library targets remain
unchanged.

The representative CMake command built both frameworks, unchanged test_bitstream
and standalone test_async_conversion_plan_rust on Linux. All four images passed
CodeDirectory page verification. Both cores export exactly 1,904 symbols, with
zero differences. The probe bundle contains these four images, 11 signed/thinned
non-system dependency dylibs and six CTest templates; both test executables embed
the producer build root, which the relocation guard correctly records.
LLVM's loader audit found only system absolute paths in the 15 staged images.

The native-contract Application category was independently cross-linked with
`--features libobs-integration --test obs-tests-app --no-run`, then collected
using #5008's package/cwd/build-script manifest helper. Its roughly 20 MB arm64
executable passed signature verification. This proves prebuilding the executable,
not that executing its existing compile-at-runtime body is allowed on macOS.

Local tooling was soldr 0.9.53 with LLVM 21.1.5 and SDK 14.5; the CI-pinned soldr
0.9.45 and hosted execution were not replayed for these CMake artifacts. The full
selected Tier 2 target remains blocked by the listed runtime/toolchain gaps.
