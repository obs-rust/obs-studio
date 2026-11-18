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
cmake -S cmake/cross -B build_macos_tier2_cross -G Ninja \
  -DOBS_CROSS_TARGET=mac-arm64 -DOBS_CROSS_TIER2=ON
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
relocate those strings. Use an agreed writable path (for example /tmp/obs-tier2-src)
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

## Blockers and fixes before activation

| Blocker / evidence | Required fix |
| --- | --- |
| Runtime CMake compilation: registered test_core_module_manifest_source passes TEST_C_COMPILER and TEST_TOOLCHAIN_FILE; check_core_module_manifest_source.cmake configures and builds a fixture. It appears in all four CTest passes. | Split fixture preparation to Linux and transfer every build result plus recorded build-graph assertions. Keep the existing runtime assertions; do not skip this registry entry. |
| Runtime Cargo-native compilation: libobs.rs contract calls build_pair, build_metal, build_macos_capture and probe builders while executing. | Split Linux preparation from macOS identity/loading/data assertions. A no-run Rust executable is necessary but insufficient. Keep all native/CMake export comparisons and negative controls. |
| Swift/Metal: libobs-metal is Swift, and swift_macos.rs requires explicit SWIFTC/SDKROOT off Darwin; no Swift compiler is provisioned by this soldr C-family toolchain. | Provision an approved Linux-hosted Swift cross toolchain plus Darwin standard libraries/overlays compatible with the blessed SDK, then build both Metal implementations on Linux. Do not compile it in a macOS preparation job. |
| Native CMake requires Xcode and SDK 26.5; soldr's blessed cross SDK is 14.5. | Use the Ninja cross graph, validate availability/weak-linking for every selected host API, and resolve SDK/Swift overlay compatibility explicitly. Do not claim the Xcode app graph can run on Linux. |
| Initial cross registry: 631 tests versus 656 in the supplied native log; 30 native names absent, five newer names added. Missing closures are obs-outputs/RTMP/AMF, happy-eyeballs, media-playback, image-source/slideshow, filters/LUT and text-file owners. | Add the real owner/dependency graphs and compare test names/properties on the same commit with native, not merely counts from different commits. No workflow switch with a partial registry. |
| Core-module list in the old cross link is empty (`OBS_CORE_MODULE_COUNT=0`), while test_core_module_manifest asserts required output/transition modules. | Generate the canonical enabled-module list from the actual selected plugin graph; do not fabricate a list to satisfy the test. |
| Qt code generators are Darwin binaries in the target package; first full build failed moc with exit 126. | Use the matching pinned Linux host Qt tools (prototype selects host moc/uic/rcc); keep Darwin Qt link/runtime artifacts and all optional test owners. |
| Darwin sanitizer runtime missing: a direct `-fsanitize=address,undefined` link fails with `cannot open .../lib/darwin/libclang_rt.asan_osx_dynamic.dylib` and undefined `__asan_init`. The native registry enables per-test sanitizers. | Provision matching Darwin compiler-rt runtimes with soldr's LLVM, sign/stage them, then replay every sanitized fixture. Disabling sanitizers would remove existing coverage. The full prototype build was interrupted after this independent failure was established. |
| Relocation includes compiled fixture paths, generated data, Qt plugins/frameworks and third-party dylib closure. | Establish a common source root before compiling; rewrite CTest build paths; stage/verify full loader/data closure. Prove selected core with DYLD_PRINT_LIBRARIES after relocation and run all four passes on macOS. |
| Signing after any install-name/load-command edit invalidates page hashes. | Set final install names/RPATHs at link; keep/thin existing signed arm64 dependencies without byte edits. Verify every executable/dylib/bundle after its final content is fixed. No entitlements are needed for plain loader probes; release signing is outside this prototype. |

No ObjC or Apple framework link failure has yet been established for libobs:
its first cross-link succeeded. Swift availability, runtime compilation and graph
completeness are distinct blockers; they should not be described as ld64.lld
failing to link Metal without an actual Swift artifact and linker diagnostic.

## Observed prototype results

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
