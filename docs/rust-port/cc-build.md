# Cargo-driven OBS native build conversion

Tracking issue: <https://github.com/obs-rust/obs-studio/issues/1332>.

The requested final state is a working OBS Rust application built through Cargo
and `cc::Build`, including shared libraries, plugins, generated files, graphics,
Qt, resources, tests and platform packaging. A core-only archive or test fixture
does not complete the conversion.

## Earlier prototype wrap-up

Full conversion work resumed at the maintainer's request on October 21, 2026.
The following records the earlier prototype deferral, not a limit on the resumed
scope: all requirements in #1332 still apply.

The maintainer requested stopping further expansion and deferring remaining
C/Rust parity validation when it delays prototype launch. Follow-up
[#1390](https://github.com/obs-rust/obs-studio/issues/1390) owns that remaining
validation. Existing tests remain intact; this is not a claim of complete
parity, release readiness or completion of the full conversion in #1332.

## Linux development application command

From a native Linux checkout with the existing native build dependencies installed:

```sh
./native-build build --profile relwithdebinfo
./native-build --help
```

The launcher builds the `obs-build` tool through `soldr cargo`, then runs it
outside Soldr's build process. Both Rust build invocations use the existing
native-consumer `SOLDR_RUSTC_WRAPPER=none` safeguard; standalone C/C++ compiler
caching remains available. The tool builds the production hybrid core,
frontend API, OpenGL renderer, Qt application, enabled modules and scripting
bindings through the Rust build graph. It does not build a C reference or ship
compatibility probes. CMake remains the authoritative build while the full
acceptance requirements above are incomplete.

Run the executable printed by the command. Each invocation stages a fresh
`build_cargo/<profile>/runtimes/<id>/` development runtime and writes
`application-build.json` there. The executable and modules search this runtime's
`lib64` before their original build directories; launching does not require
`LD_LIBRARY_PATH`. This layout still relies on the host's native SDK/runtime
libraries and is not a distributable package.

Profiles are `debug`, `release`, `relwithdebinfo` (default) and `minsizerel`.
`--separate-core` and `--core-pch` select core compilation modes. Help lists the
optional plugin/SDK and scripting selections. Native Linux is the only full
application target currently accepted by this command; the macOS/Windows graphs
below are intermediate component coverage. Complete configuration parity,
all-platform application orchestration, packaging and the controlled native
cache/performance acceptance remain open.

NVENC and its capability helper are enabled by default, matching CMake. The
command builds only the existing Rust CUDA loader for production. Install
`ffnvcodec >= 12` headers, or select `--without-nvenc` to omit the module,
helper and Rust archive. `--without-nvenc-ffmpeg-ids` keeps NVENC enabled but
omits its legacy FFmpeg encoder aliases. The manifest records this selection.
The NVENC source inventory is shared with CMake through
`plugins/obs-nvenc/build/source-sets.json`.

On a host without usable NVIDIA driver libraries/devices, the helper reports
`nvenc_supported=false` and OBS logs `NVENC not supported`; software encoders
remain available. The application runtime test checks this outcome separately
from module loading on supported hardware. A software recording does not prove
hardware NVENC encoding.

Frontend source groups are shared by CMake and Cargo through
`frontend/build/source-sets.json`; the Qt wrapper host groups similarly use
`shared/qt/wrappers/source-sets.json`. Generated configuration stays in the build
output directories.

The existing application test category can validate the exact command-produced
runtime, rather than rebuilding its application artifacts:

```sh
OBS_BUILD_APPLICATION_RUNTIME=/absolute/path/to/runtime \
OBS_BUILD_APP_RECORD=1 \
soldr cargo test -p obs-studio-rust --test obs-tests-app \
  --features libobs-integration production_command_runtime_runs_saved_scene_and_recording \
  -- --nocapture
```

Run with an isolated X display (for example Xvfb) and the existing GUI test
dependencies, including the FFmpeg executables. The test owns a fresh profile in
the runtime directory: startup, the selected staged libraries/modules, saved
scene, Lua/Python callbacks, recording and normal shutdown are asserted. Use a
freshly staged runtime for every test invocation. The separate compatibility
driver retains its C-reference/hybrid, Tools dialog and WebSocket checks.

## Default build mode

The user selected **unity on, compiled PCH off**. Keep both settings optional.
`libobs::Options::default()` uses that mode; bundled `obs-sys` exposes the same
choice as `OBS_SYS_UNITY=1` and `OBS_SYS_PCH=0` when no override is supplied.
Components that already list explicit unity groups preserve those boundaries.

## macOS core graph

The native engine selects macOS core sources from the same
`libobs/build/source-sets.json` used by CMake. It builds an explicit C reference
and the production Rust/C hybrid, retaining the framework install name and
macOS 13 deployment target. The generated development framework lookup layout
does not supply release metadata, signing, or application packaging.

Set `libobs::Options::native_dependencies` to an explicit prebuilt OBS dependency
bundle. Bundled `obs-sys` accepts the same root through
`OBS_SYS_NATIVE_DEPENDENCIES`, tracking that input for Cargo rebuilds. The outer
Cargo invocation supplies the release Rust archive via `OBS_BUILD_RUST_ARCHIVE`.
Cross compilation additionally needs a target compiler, Apple sysroot and Darwin
linker; the graph does not infer these from host package discovery.

`SourceCompileOptions` applies source overrides after target/language flags and
before PCH generation. `NativeBuild::separate_target_sources` carries those
overrides and explicit PCH assignments to individual group members. This keeps
ARC cocoa groups separate from the non-ARC loader even when they share a header.
Clang PCH now supports macOS as well as Linux; both unity and PCH remain optional.

The existing macOS Tier 2 script also builds the Cargo core pair, compares its
exports to the actual CMake reference, proves dyld selected each Cargo core, and
runs the unchanged CTest suite against each. It checks the bundled crate's build
path separately. These are core checks: they do not establish the macOS frontend,
graphics backends, plugins, complete application, or release package conversion.

## macOS graphics graphs

The macOS OpenGL graph uses the canonical graphics manifest, the original C and
Cocoa groups, GLAD and the Cocoa/IOSurface/OpenGL frameworks. The Cocoa group
keeps manual reference counting. Unity and compiled PCH follow the core options
independently. ARM64 cross builds produce 153 exported symbols; actual CMake
export agreement and native loading require the macOS validation lane.

`graphics::build_metal` consumes all 34 original Swift files and the original
bridging header from the same manifest as CMake. `NativeBuild::swift_shared_library`
accepts typed `SwiftLibrary` inputs, invokes the Swift compiler and emits the
dylib, Swift module and Objective-C compatibility header under the build output.
C-family compilation continues through `cc::Build`. Metal uses Swift 6, optimized
whole-module compilation, debug information and the existing macOS 13 minimum.
Release/configuration overrides and distribution signing remain separate work.

Native Apple builds discover `swiftc` and the macOS SDK through `xcrun`. Override
them with executable-path `SWIFTC` and SDK-directory `SDKROOT`; target-qualified
forms (hyphens or underscores in the triple) take precedence. Cross builds must
supply both paths and an Apple-compatible Swift toolchain. The Clang importer
receives the declared include paths and C definitions. Swift sources, the bridge,
declared native inputs/dependencies and tool-selection environment are tracked;
module caches live beside the generated library. Cache placement alone does not
establish compiler-cache hits or a controlled performance result.

The existing native-build category contains a Swift/C contract requiring results
42, 52, 54, 56 and 58 after header, importer flag, Swift source and C dependency
changes. Each result is checked in a fresh process, preserving dyld's Objective-C
image lifetime behavior. This contract executes on native macOS, not Linux.

`OBS_BUILD_MACOS_GRAPHICS=1` extends the existing core integration contract to
both renderers; `opengl` selects an explicit cross-build subset and does not
establish Metal coverage. `OBS_BUILD_MACOS_OPENGL_REFERENCE` and
`OBS_BUILD_MACOS_METAL_REFERENCE` require exact exports from actual CMake modules.
The macOS Tier 2 script builds those originals and both Cargo renderers, then
checks core/module file identity and public device name/type in fresh processes
against each Cargo core. Wrong-core and wrong-backend controls must fail. The
unchanged CTest suites remain. These loading checks do not create a graphics
device, verify rendered pixels, record media or prove the full application.

## macOS capture module graphs

`LibraryKind::Module` selects a loadable module: Apple linking uses `-bundle`
and omits the dylib install name, while shared libraries retain `-dynamiclib`.
The capture contract verifies `MH_BUNDLE` headers in both Cargo and CMake
artifacts. The existing shared-core/plugin fixture exercises module loading
and observes a rebuilt dependency without relinking the module.

`plugins::build_macos_capture` builds the original `mac-capture`,
`mac-avcapture` and `mac-avcapture-legacy` modules. CMake, its cross checks
and Cargo consume `plugins/mac-capture/source-sets.json`. The screen/audio
Objective-C group keeps manual reference counting; the modern camera uses
ARC and Clang modules, and the legacy camera keeps Objective-C++ with ARC.
Framework dependencies, weak ScreenCaptureKit linking, macOS 13 minimum,
locale data and independent unity/PCH choices are retained. Each development
artifact has the native `.plugin/Contents/MacOS/<name>` layout, staged resources
and an `Info.plist` with its executable and bundle identifier. A snapshot of
binary, resource paths/contents and metadata selects the output directory so
removed resources cannot survive from an older same-named bundle. Release
version metadata, signing and distribution packaging remain incomplete.

`OBS_BUILD_MACOS_CAPTURE=1` extends the existing core integration contract.
The three `OBS_BUILD_MACOS_MAC_CAPTURE_REFERENCE`,
`OBS_BUILD_MACOS_MAC_AVCAPTURE_REFERENCE` and
`OBS_BUILD_MACOS_MAC_AVCAPTURE_LEGACY_REFERENCE` inputs require exact export
agreement with actual CMake binaries. The native macOS Tier 2 script builds
those references and runs each Cargo/CMake module with both Cargo cores in
fresh processes. Checks cover module/core file identity, API version,
description, byte-for-byte resource agreement, CoreFoundation bundle/executable identity,
initialization and all registered source IDs.
Wrong-module and wrong-core controls must return 14 and 12 respectively.
The unchanged CTest suites still run against both cores.

Local ARM64 cross builds pass with unity on/PCH off and separate sources/PCH on.
The Linux cross toolchain lacks Apple's availability runtime; the local check
links LLVM 21.1.5's original `compiler-rt/lib/builtins/os_version_check.c`,
compiled for the target, without altering source availability checks. Native
Apple Clang supplies its own runtime. Cross compilation does not establish
native module initialization. Native execution/export agreement remains a
required CI result. Registration checks do not open screen, microphone or
camera devices, verify captured media, or prove packaged saved-profile behavior.

## Windows core graph

The Windows MSVC graph uses the shared libobs source manifest and existing
`rust-exports.d` fragments. The obfuscation interface group is also selected
from that manifest by CMake. An explicit OBS dependency bundle supplies headers
and import libraries; `w32-pthreads` is built from its existing source and
resource template. Each core retains `obs.dll` and its `obs.lib` import library.

`SharedLibrary::resource_files` contains native `.rc` scripts. Declare their
included headers, icons and other inputs in `resource_input_files` for Cargo
tracking; `resource_compile_options` supplies RC-specific options. The resource
compiler receives the target's definitions and include paths, separately from
C/C++ flags, unity and PCH. Set a target-qualified `RC_<triple>` (hyphens or
underscores), `HOST_RC`/`TARGET_RC`, or `RC` to an executable path. Otherwise the
engine discovers the Windows SDK resource compiler, falling back to `llvm-rc`
for clang-cl. Resources link into DLLs or executables; putting resource scripts
in a static archive is rejected instead of silently losing them.

MSVC and clang-cl PCH use the same language and source-specific flags as their
consumers. PCH-generated objects are retained for linking and archiving, with
shared common/variant objects linked only once. The native contracts cover
resource payload changes, DLL/executable loading, C/C++ flag isolation, header
changes, unity modes and PCH objects linked through a static archive.

The existing Windows Tier 2 script builds the Cargo pair with PCH enabled,
compares exports to its CMake reference, and checks bundled `obs-sys` with PCH
disabled. A separate loader probe checks the loaded core's file identity and a
deliberately incorrect expected file before the unchanged CTest suite runs
against each Cargo core. Test-specific runtime paths are checked for shadowing.
Cross builds can use `OBS_BUILD_WINDOWS_DEPENDENCIES`, `OBS_BUILD_WINDOWS_TARGET`,
`OBS_BUILD_WINDOWS_OUTPUT`, and `OBS_BUILD_WINDOWS_RUST_ARCHIVE` with the existing
`native_libobs::windows_tests::core_builds_from_the_canonical_manifest` test.
`OBS_BUILD_WINDOWS_PCH`, `OBS_BUILD_WINDOWS_SEPARATE` and
`OBS_BUILD_DISABLE_HEVC` select additional configurations. Export inspection uses
`OBS_BUILD_WINDOWS_EXPORT_TOOL` (llvm-readobj by default, or dumpbin on Windows).

These are core checks. Resource metadata currently follows the core API version
and existing bootstrap strings; full configuration/release-version overrides,
frontend, graphics, plugins, packaging and controlled compiler-cache measurements
remain required before retiring CMake authority. Cross-built artifacts do not
prove Windows runtime behavior.

## Windows graphics graphs

`graphics::build_d3d11` and `graphics::build_opengl` build the original Windows
renderers against a selected Cargo core. Direct3D uses C++17, the original SDK
libraries and a selectable integer GPU thread priority (zero by default).
OpenGL retains the GL/WGL loader objects and system OpenGL linkage. Both renderers
compile their existing Windows version-resource templates. Release metadata
unification remains required; these resources currently follow the core API
version and bootstrap strings, as the Cargo core does.

The canonical `cmake/graphics-source-sets.json` supplies graphics/GLAD sources to
the original CMake targets, cross checks and Cargo. The existing unity groups
remain in source order; core `Options::unity` and `Options::pch` also select
renderer compilation. GLAD retains separate translation units. Its generated GL
and WGL loaders share a tentative C global; CMake and Cargo both request common
symbols for clang-cl, matching MSVC without editing the generated sources.

Set `OBS_BUILD_WINDOWS_GRAPHICS=1` on the existing Windows core integration
contract to build both renderers. `OBS_BUILD_WINDOWS_GPU_PRIORITY` selects the
D3D11 priority. `OBS_BUILD_WINDOWS_D3D11_REFERENCE` and
`OBS_BUILD_WINDOWS_OPENGL_REFERENCE` require exact exported symbol agreement with
actual CMake renderers. Cross builds produced 171 D3D11 and 151 OpenGL exports
and version-resource sections; they do not prove native loading or rendering.

The Windows Tier 2 path also builds the original CMake renderers and compares
their exports. A fresh-process native probe checks the loaded renderer's file
identity, public device name/type and version-resource description against each
Cargo core, with a wrong-renderer negative control. The same probe exercises the
CMake modules against those cores. These checks do not create a graphics device,
verify rendered pixels or establish complete Windows application coverage.

## Requirements carried forward

- Native C/C++ builds must be very fast. Reuse explicit unity groups, make unity
  configurable, and implement optional PCH with compiler-compatible settings and
  correct dependency invalidation. The user's instruction that unity and PCH are
  optional supersedes BUILD.md's always-on rule for this conversion.
- Build through soldr. Demonstrate real compiler cache hits, cold/warm timings,
  no-change builds and correctness after editing source, headers and generated
  inputs. A setup-soldr archive restore alone does not prove native cache reuse.
- Preserve the public C ABI, dynamic loading, plugin discovery, library identity,
  exported symbols and the existing Rust implementation. Build both libobs
  variants and exercise the unchanged compatibility and differential suites.
- Keep the application and bundled crates on one native build description.
- Generate files in build output directories; explicitly consume generated
  outputs so stale source-tree files cannot shadow them (PR #1323).
- Preserve the complete feature surface on Linux, Windows and macOS. Verify a
  real running application, scenes, recording, plugins, shutdown and applicable
  websocket tests. Packaging validation must run packaged artifacts.
- Ordinary Cargo users can provide documented native tools; soldr manages the
  toolchain and caching. Do not require a nested soldr invocation inside build.rs.
- Reuse existing CI workflows; no new workflow files are authorized.

## Implementation order and evidence

1. `obs-build`: typed native build inputs, actual cc::Build compilation, shared
   linking, plugin loading and unity on/off contracts. Start with failing tests.
2. `obs-sys`: the real libobs graph, native dependencies, generated configuration
   and Rust twin; unchanged C tests and symbol parity.
3. Frontend/plugins, Qt code generation and resources, platform linking and
   packaging; run the existing application test battery on these artifacts.
4. Optional PCH, cache/incremental performance measurements and existing CI
   integration. Retire CMake only when complete feature/test parity is evidenced.

The existing CMake build remains the artifact and behavior reference during the
conversion. Intermediate progress must not be reported as full completion.

## Linux libobs checkpoint

The bundled `obs-sys` feature now compiles the real C libobs through `cc::Build`,
links `libobs.so.30`, and generates configuration, version and core-module source
files under Cargo's output directory. It uses the same canonical source manifest
as the CMake reference. libcaption remains a separate static archive; system
libraries remain dynamically linked, including when pkg-config lists a static
archive directory before its shared-library directory.

Local evidence for this checkpoint:

- The native build engine's three tests pass (dependency selection, generated
  source/unity toggle, and a dynamically loaded plugin observing a rebuilt core).
- Bundled libobs passes its public C API version and startup/shutdown tests.
- Editing only the JSON manifest triggers CMake regeneration. Changing a
  target-qualified C compiler flag reruns Cargo's bundled native build; compiler
  metadata does not add unintended static-library linkage.
- Its 1,766 exported symbols exactly match the local CMake C reference.
- 51 unchanged, already-built C test executables pass against both libraries.
  `ldd` confirms each executable loads the intended replacement `libobs.so.30`.
  This validates the replacement library; it does not demonstrate a Cargo build
  of those test executables or the Rust implementation twin.

This checkpoint is Linux C libobs only. The application and plugins,
full test battery, macOS/Windows graphs, installation/packaging, optional PCH and
controlled build/cache benchmarks remain required. The generated version and
install paths are prototype defaults and still need release configuration.
Native dependencies are discovered using pkg-config; local Nix verification used
`PKG_CONFIG_PATH` and `CPATH` for installed packages and uthash respectively.


## Linux Rust implementation twin checkpoint

`libobs::build_pair` accepts the release `libobs-rust` static archive from an
outer Cargo invocation. It compiles common C objects once, then links the C
reference and production hybrid into separate directories with the same SONAME.
The Rust archive is whole-archive linked and its internal symbols are hidden by
the existing export map. Only the hybrid is installed; the C reference is explicit testing infrastructure.

Build the release archive with `soldr cargo build -p libobs-rust --release`.
With native dependencies available, set `OBS_BUILD_RUST_ARCHIVE` to the absolute
`target/release/liblibobs_rust.a` path and run
`soldr cargo test -p obs-studio-rust --features libobs-integration --test obs-tests-app`.
`OBS_BUILD_TEST_OUTPUT` optionally retains artifacts in a chosen output directory.
The test defaults to a temporary output directory. Set
`OBS_BUILD_DISABLE_HEVC=1` to exercise the disabled-HEVC configuration. GNU `nm`
is required for the Linux export comparison.

Local evidence: both variants pass startup/shutdown and public data API roundtrip
checks, and each exports exactly the same 1,766 symbols (1,762 with HEVC disabled).
The opt-in test compares complete export sets and checks HEVC export availability.
The same 51 unchanged C
executables pass against both newly built variants with their loaded library
paths checked. The rest of the complete application/platform/test/packaging
requirements above remain outstanding.


## Linux transitions and filters checkpoint

`plugins::build_transitions` and `plugins::build_filters` build actual OBS
modules through the native engine and link them to the shared C library.
The filters options preserve independent SpeexDSP and RNNoise switches. The
bundled RNNoise fallback uses a canonical JSON C source list shared with CMake;
the CMake target retains the same 18 source/header entries.

The opt-in integration test loads these modules through `obs_open_module` and
`obs_init_module`, then verifies transition/filter registration against both
libobs variants. Each variant runs in a separate process: the shared SONAME
allows plugin dependencies to retain a previous libobs instance if both variants
are tested in the same process. This matches existing C test isolation and normal
OBS execution. These checks do not yet exercise rendering or audio processing.


All four SpeexDSP/RNNoise combinations pass module registration against both
variants, as does disabled HEVC with the default modules. The actual CMake
RNNoise fallback target also compiles all seven objects after the canonical-list
migration; Clippy, gersemi and the build-group/platform guards pass. System
RNNoise discovery is implemented, but this host exercises the bundled fallback.


## Linux OpenGL checkpoint

`graphics::build_opengl` compiles the original OpenGL unity groups and GLAD loader,
links the shared backend, and creates its unversioned lookup name. It consumes the
same libobs options and generated configuration as the core. The Linux graph
includes X11/EGL and optional Wayland support; other platforms remain unfinished.

With an isolated X server available through DISPLAY, set `OBS_BUILD_RENDER=1` on
the opt-in integration test to exercise real graphics. Local verification used an
owned Xvfb process allocated via `-displayfd`, with `LIBGL_ALWAYS_SOFTWARE=1` and
`GALLIUM_DRIVER=llvmpipe`. Both variants initialize Mesa OpenGL 4.6, create a scene,
and receive at least three 64x64 NV12 frames through the public raw-video callback
API. The callback verifies a valid row and the expected black Y sample (16).
The probe uses the unchanged C headers for video configuration and callbacks;
shutdown and display cleanup happen before its shared libraries unload.

This demonstrates the native renderer and basic frame delivery. It does not yet
validate the full Qt application, recording, capture sources, all plugins,
Wayland runtime behavior or packaged artifacts. The prototype OpenGL version
number remains tied to the current OBS 33 configuration.


## Linux recording checkpoint

The native engine now links executables as well as shared libraries and archives.
Its executable contract runs a program linked to a shared core that consumes a
static helper. Windows dependency mapping preserves archive paths and converts
DLL paths to import-library paths; runtime validation there remains outstanding.

The module graph builds x264, the complete Linux obs-outputs groups and librtmp,
and FFmpeg with media playback, VAAPI, optional MPEGTS/SRT/RIST, logging and legacy
NVENC groups. It also builds the standalone FFmpeg mux executable. Shared
opts-parser, happy-eyeballs, BPM and libcaption dependencies use native archives.
The FFmpeg options follow current CMake defaults; the full option matrix and
hardware-specific behavior remain unverified.

Set `OBS_BUILD_RECORD=1` on the opt-in integration test, with native dependencies,
ffprobe and an isolated DISPLAY available. This implies the graphics probe. Both
variants load the encoder/output modules, verify registration and exactly one
loaded SRT library, then encode an isolated blank scene using real obs_x264,
ffmpeg_aac and mp4_output. The test stops the output, releases encoders, shuts
down OBS, and validates tracks, dimensions, decoded-frame counts and duration
with ffprobe. Local recordings each contain 18 H.264 frames at 64x64 and AAC
audio, duration 0.6 seconds. They contain no desktop or device capture.

`FfmpegArtifacts::deploy_muxer` places the helper beside the running app. The
integration stages its runner with that helper, selects the same libobs variant
for helper subprocesses, and also records through `ffmpeg_muxer` to Matroska.
All four files (MP4 and Matroska, C and Rust cores) pass the same ffprobe checks. The full Qt application, capture sources, other
modules/platforms, complete test battery, packaging and controlled performance
measurements remain required. Do not treat this checkpoint as a CMake retirement.

## Linux sources and services checkpoint

`plugins::build_source` provides typed selections for image sources, FreeType
text, PulseAudio, ALSA, V4L2, X11 capture and RTMP services. V4L2 preserves its
independent UDEV switch. RTMP service updates are configurable and remain off in
the integration smoke test to keep it offline. The default plugin behavior keeps
updates enabled when requested through its typed option. X11 capture and OpenGL
share the GLAD archive builder.

Set `OBS_BUILD_SOURCES=1` on the integration test with an isolated X display and
an EWMH window manager. Local checks use Xvfb plus an isolated KWin session;
XComposite correctly declines registration on bare Xvfb without EWMH. Both cores
load these seven modules and expose their thirteen expected input/output/service
registrations. The graphics probe also creates a 64x64 white color source,
adds it to the scene and requires three NV12 frames with Y=235 after draining
previous black frames. Both cores still record validated MP4 and Matroska files.
Disabled UDEV also passes registration and rendering against both cores.
The test does not open camera or microphone devices. Hardware capture, text
rasterization, streaming and service-update behavior remain to be validated with
the full application and packaged artifact tests.

## Linux PipeWire checkpoint

`SourceModule::PipeWire` builds the original Linux PipeWire unity group and
includes the camera group when the discovered PipeWire version is at least
0.3.60. It enforces CMake's PipeWire 0.3.33 and Gio 2.76 minimums and uses the
shared GLAD archive. Enable its opt-in build/load check with
`OBS_BUILD_PIPEWIRE=1`. Both cores load and initialize the module after graphics
initialization, then render and shut down. Portal availability, camera
permissions and actual screen/camera capture remain unverified; module load alone
does not establish those capabilities.

## Linux frontend API and Qt checkpoint

`frontend::build_api` builds the original C++ frontend API as a shared library.
It preserves Linux's SONAME 30 and creates its unversioned lookup name. C++17 and
hidden inline visibility match the CMake compiler configuration. Local symbol
comparison finds exactly 101 exports in both frontend libraries, with no missing
or extra symbols. `OBS_BUILD_FRONTEND_REFERENCE` optionally enables this complete
comparison against an existing CMake artifact in the integration test.

`qt::moc` runs Qt's meta-object compiler with the native target's include paths and
definitions and compiles its generated translation unit explicitly. It preserves
unchanged output contents/mtime. Host tools come from explicit Qt SDK inputs
(see [Explicit Qt SDK inputs](#explicit-qt-sdk-inputs)). Application builds
still need UI/resource generation and complete build-script input tracking.

Enable `OBS_BUILD_QT=1` to build the real VST module and frontend API. Both cores
load the VST module and expose its filter; the frontend API's no-callback defaults
are callable. VST editor/audio behavior, the full Qt application and resources,
other platforms and packaging remain outstanding.

## Mixed native languages checkpoint

Native targets now accept common compiler options plus separate C and C++ option
vectors. OBS modules use C17 for C sources and C++17/hidden inline visibility for
C++ sources. Mixed targets compile each language independently and link with the
C++ driver. The regression builds and executes a C++ standard-library program
that calls a C helper; each translation unit rejects flags intended for the other
language. All five native integration tests pass, as does the combined
core/source/Qt/PipeWire/MP4/Matroska check with frontend export comparison.

## Qt UI, resources and widgets checkpoint

`qt::uic` generates UI headers with the generated include directory first.
`qt::rcc` embeds resource collections as explicit C++ sources and records every
resource listed by RCC as a Cargo input. Both preserve unchanged generated files.
`OBS_QT_UIC` and `OBS_QT_RCC` select host tools for cross builds, alongside MOC.

`frontend::add_spinbox` adds the original shared spinbox unity source, Qt Widgets
and all three generated metaobjects, with C++17 required explicitly. The Qt opt-in
integration now runs a real QApplication using the offscreen platform: construct
OBS's virtual-camera dialog from its original UI, find its controls, decode an
embedded OBS PNG, exercise both OBS spinbox types and their metaobject, process
UI events and accept the dialog through its OK button. This passes locally.
This widget test does not yet start the complete OBS application or validate a
packaged runtime/plugin path.

## Explicit Qt SDK inputs

`OBS_QT_TARGET_PREFIX` names the Qt SDK the target links, and
`OBS_QT_HOST_PREFIX` names the SDK whose `moc`, `uic` and `rcc` run on the build
host (`libexec/` on Linux and macOS hosts, `bin/*.exe` on Windows hosts).
`OBS_QT_MOC`, `OBS_QT_UIC` and `OBS_QT_RCC` still override one tool each. A
native build may give only the target prefix, which then also supplies the
tools. With neither prefix, a native build keeps the system Qt6 pkg-config
path. A cross build must name both SDKs: `qt::package` and `qt::Tools` refuse
to fall back to the host's pkg-config, which would select host libraries. The
inputs are read, never written back to the process environment.

`qt::Sdk` reads the SDK's own exported CMake targets
(`lib/cmake/Qt6*/Qt6*Targets*.cmake`) as data. CMake is never run. For a module
and its `Qt6::` dependencies it evaluates the include directories, definitions,
compile options and link libraries for the Cargo target. It evaluates only the
generator expressions Qt exports (compiler, language, configuration, unset
consumer properties) and fails on any other. `libobs::add_package` and
`Dependency::package` route every `Qt6<Module>` package through it when a target
prefix is given. Before anything compiles it checks:

- **Architecture:** the module binary's ELF, PE or Mach-O header (every slice of
  a universal framework) must contain the target architecture.
- **Version:** `QT_VERSION_STR` in QtCore's `qconfig.h` must be Qt 6, and every
  host tool's `--version` must report the same release as the target SDK.
- **Configuration and CRT:** only configurations with a
  `Qt6<Module>Targets-<config>.cmake` export count; Qt's AdditionalTargetInfo
  aliases are skipped. Rust's MSVC targets always link the release CRT, so a
  Windows SDK whose only configuration is DEBUG (`/MDd`) is rejected, as is
  `crt-static`. Multi-configuration SDKs use their release configuration.

Linux links the versioned `libQt6*.so` with an rpath to the SDK. Windows links the
import libraries (`lib/Qt6*.lib`) and the system libraries Qt exports, and
returns the DLL directory as the `PATH` runtime prepend for graph tests. macOS
links the framework binaries, Qt's exported system frameworks (each once) and an
rpath to the frameworks' directory.

The Linux, Windows x86_64 and macOS universal2 SDKs published in
`obs-rust/obs-studio-libs` (`qt6-6.11.1`) all resolve.
`qt_sdk_resolves_published_sdks` checks them when `OBS_QT_TEST_LINUX_PREFIX`,
`OBS_QT_TEST_WINDOWS_PREFIX` and `OBS_QT_TEST_MACOS_PREFIX` name the extracted
archives. With `OBS_QT_TARGET_PREFIX` set to the Linux SDK and no Qt visible to
pkg-config, the metaobject/UI/resource graph test and the unchanged remux queue
test compile and pass through Cargo and CMake. Qt is never built in CI; SDKs come
only from obs-studio-libs. Compiling against the Windows and macOS SDKs on those
hosts, Windows ARM64, MinGW and Linux distribution (multiarch) layouts as explicit
prefixes are not yet covered.

## Linux frontend executable checkpoint

`frontend::build_application` compiles and links the frontend executable using an
explicit source inventory, shared Qt interfaces, an Idian archive, JSON11, BPM,
libcaption, the frontend API and a caller-provided Rust GPU status archive. It
creates UI/plugin-path configuration in the output tree. This initial frontend
configuration is Linux with browser panels disabled; other feature/platform
configurations and release paths remain under conversion.

Build the GPU archive with
`soldr cargo build -p obs-frontend --release`, then enable the integration
check with `OBS_BUILD_APP=1` and `OBS_BUILD_GPU_ARCHIVE` pointing to the resulting
`target/release/libobs_frontend.a`. The executable runs and reports
`--version` identifies the OBS-Studio-Rust application version as `0.1.0`,
based on OBS `33.0.0-cc-build`.
This establishes compilation, linking and CLI execution; GUI startup, runtime
staging and complete application behavior still need validation.

The inventory includes two forms that CMake AUTOUIC discovered through header
includes. Generated metaobjects are combined into one translation unit with the
same textual header closure CMake supplies to its moc compilation. This preserves
complete Qt types without changing original headers or requiring precompilation.
Compiler include paths and identical definitions are deduplicated while retaining
order. Build/profile/version/install settings are still prototype defaults, and
controlled performance measurements have not been completed.

## Linux staged GUI runtime checkpoint

Set `OBS_BUILD_APP_GUI=1` with `OBS_BUILD_GPU_ARCHIVE`, native dependencies,
an isolated X display and an EWMH window manager to run the complete current
Linux graph. This implies the frontend, Qt widget probe, fourteen implemented
modules, OpenGL, source rendering and the MP4/Matroska API recording checks.

The GUI test stages the executable, selected core, frontend API, renderer,
modules, shaders, application/plugin data and FFmpeg helper under a retained
`app-runtime-*` directory in the native output tree. Each core starts in a fresh
process with its own saved profile and white color-source scene. The test checks
startup, a mapped normal main window, the staged library and module paths in
the process map, restored source, screenshot, normal window-close shutdown and
persisted scene. Logs, process maps and screenshots remain in that runtime.
Both C and Rust cores pass these checks locally; both screenshots show the
restored white preview. The combined run also validates four H.264/AAC files
(MP4 and Matroska for each core). This is runtime evidence, not a controlled
build-performance comparison.

The isolated test clears desktop-session Qt plugin paths and themes so a host
desktop built with another Qt installation cannot override the linked Qt's
platform plugins. It retains the linked Qt's default plugin discovery and uses
Fusion styling. System native dependencies remain installed on the host; this
development layout does not establish portable release packaging. Hardware
capture, browser/websocket features, remaining modules/platforms,
full test tiers and controlled performance measurements are still required.

Add `OBS_BUILD_APP_RECORD=1` alongside `OBS_BUILD_APP_GUI=1` to exercise the
frontend's real recording path. Its private profile selects x264/AAC and
Matroska at 640x360/30fps; `--startrecording` starts the frontend output. The
probe requires the recording-start log, records for three seconds and closes
the window normally. ffprobe must decode at least sixty video frames and find
H.264, stereo AAC at 48kHz and the expected dimensions/duration. FFmpeg decodes
the first video frame and checks that it contains the restored white scene.
The finalized recording and ffprobe JSON remain under the retained runtime.
Both cores pass locally: each file has 98 decoded H.264 frames, 48kHz stereo AAC
and a 3.366-second container duration. Both screenshots show recording active
with the white preview. The private headless session's desktop portal backend
does not remain healthy; these checks establish no screen/camera portal capture.

## Linux NVENC checkpoint

`plugins::build_nvenc_pair` compiles common NVENC objects once, then links the C
CUDA loader and Rust CUDA loader variants separately. The latter accepts the
outer Cargo build's `obs-nvenc-plugin` archive and hides its Rust symbols. The
capability helper is a separate C++17 executable with thread/dynamic-loader
linkage. FFnvcodec headers must be version 12 or newer. The FFmpeg compatibility
encoder IDs remain independently configurable.

Build `soldr cargo build -p obs-nvenc-plugin --release` and set
`OBS_BUILD_NVENC=1` on the native integration test. This opt-in check requires
a working NVIDIA driver/GPU: the helper must report support, and both plugin
variants must load and register their H.264 encoders against their corresponding
core. `OBS_BUILD_NVENC_RUST_ARCHIVE` overrides the default archive location;
`OBS_BUILD_NVENC_REFERENCE` optionally compares all exports to a CMake module.
The local comparison finds exactly eight exports in all three modules.
`OBS_BUILD_DISABLE_NVENC_FFMPEG_IDS=1` disables the compatibility IDs.

Add `OBS_BUILD_NVENC_ENCODE=1` with recording mode to exercise the soft-frame
hardware encoder and therefore the actual CUDA driver loader. This uses 640x360
because NVENC rejects the tiny 64x64 software probe. Both variants produce
validated H.264/AAC MP4 and Matroska files locally. Source cleanup explicitly
removes public sources from their canvas, releases owned references and drains
the destruction queue before shutdown, matching the frontend sequence.

Nix verification provides the installed driver libraries through
`LD_LIBRARY_PATH=/run/opengl-driver/lib`. This checkpoint covers native module
loading and soft-frame H.264 encoding. OpenGL texture interoperability,
HEVC/AV1 encoding and other platforms still
need validation.

## Linux staged GUI NVENC checkpoint

Combining `OBS_BUILD_NVENC=1`, `OBS_BUILD_APP_GUI=1` and
`OBS_BUILD_APP_RECORD=1` now stages fifteen modules and the NVENC capability
helper. Explicit runtime variants select the C loader with the C core and the
Rust loader with the Rust core. The core's bundled-module whitelist includes
NVENC when this option is requested.

The private frontend profile selects NVENC. In addition to the existing GUI
checks, the test requires the staged NVENC module in the process map and the
actual recording encoder's NVENC settings log. Both variants pass locally,
producing 99/100 decoded H.264 frames at 640x360 with AAC audio and white images;
screenshots show recording active and both apps shut down normally. On the
isolated Mesa display, NVENC reports its expected fallback to frame upload.
This demonstrates frontend hardware encoding through both loaders; it does
not establish direct NVIDIA OpenGL texture interop.

## Linux Lua/Python scripting checkpoint

`scripting::build` compiles the original shared scripting unity groups and
C++ string cache. Lua and Python remain independent options, and a supplied
frontend API enables the original frontend scripting groups. SWIG 4 or newer
generates separate language runtimes and wrappers; `OBS_SWIG` selects the host
tool. The build tracks SWIG environment/input trees, writes generated sources
and headers only when their contents change, and explicitly compiles them.

Bindings are placed under a retained runtime root with a configurable relative
module directory (default `lib64/obs-scripting`). The scripting library uses
executable-relative discovery. Artifacts identify the enabled Lua module,
Python extension and Python wrapper explicitly; disabled-language files from
an earlier build are not an installation manifest. Release packaging must use
the selected artifacts.

Enable the opt-in check with `OBS_BUILD_SCRIPTING=1`. It builds a native probe
against the unchanged scripting API, then runs it with each core from an
unrelated working directory and private module configuration. Real Lua and
Python scripts exercise defaults, update, properties, save, reload and unload
callbacks. Process maps must contain the intended core and generated binding,
and the supported-language list must match the build options. Logs remain in
`script-scenarios/` under the native output directory.

`OBS_BUILD_SCRIPTING_FRONTEND=1` enables frontend bindings and verifies their
default callable behavior; the disabled mode checks that they are absent.
`OBS_BUILD_DISABLE_LUA=1` and `OBS_BUILD_DISABLE_PYTHON=1` select the independent
language cases. Local checks pass both languages with and without frontend
bindings, Lua-only with a relative native output path, and Python-only with
frontend bindings, against both cores. The environment uses SWIG 4.4.1,
LuaJIT and Python 3.13 development packages discovered through pkg-config.

The shared-library check remains separate from the frontend Tools application
check below. Other platforms and release packaging still require conversion
and validation.


## Linux frontend Tools and saved GUI scripts

`plugins::build_frontend_tools` compiles the original scene-switcher,
output-timer and optional scripting groups, their Linux/POSIX host sources,
shared Qt widgets, generated forms and moc units. Supplying scripting artifacts
enables the matching Lua/Python UI definitions and shared-library dependency.
The graph reuses the application's shared Qt interface inventory.

Set `OBS_BUILD_FRONTEND_TOOLS=1` with `OBS_BUILD_APP_GUI=1` to stage the plugin
and enabled scripting artifacts in each private runtime. Saved Lua/Python
scripts are restored from the scene collection. The app test checks their
load/update/save/unload callbacks and persisted settings, as well as the
actual staged scripting libraries in the process map. It copies only the
selected language artifacts, excluding stale disabled-language output.

A test-only Qt module opens Automatic Scene Switcher, Output Timer and Scripts,
checks the saved script count and each script's description/Seed property,
captures each dialog and closes it. Inspection
runs inside Qt's event loop, including Scene Switcher's nested modal loop.
The probe is part of the development test runtime and is not a release plugin.
`OBS_BUILD_DISABLE_SCRIPTING=1` exercises Tools without the scripting library.
Combine `OBS_BUILD_APP_RECORD=1` to retain the actual frontend recording checks.

Local dual-language and scripting-disabled app checks pass against both cores,
including real frontend H.264/AAC recording and the four native API recordings.
The combined Tools, Lua/Python and NVENC configuration also passes against
both cores, with the selected C/Rust NVENC loaders and helper staged together.
All six dual-language dialog screenshots and the recording main window were
visually inspected. The disabled case requires the Scripts action to be absent.

Both configurations can print one outstanding allocation during shutdown in
this headless desktop session. An allocation tracker identifies the original
portal sleep-inhibitor object (`portal_inhibit_info_create`); a diagnostic
nonblocking GLib main-context drain releases its pending callback and changes
`bnum_allocs()` from one to zero. The diagnostic is external to the production
build and app probe. This checkpoint establishes normal app exit and scripting
cleanup, not a zero-allocation shutdown log or portal capture support.

Windows/macOS Tools, remaining modules, full application/capture coverage and
release packaging are still required.

## Linux websocket server and protocol checks

`plugins::build_websocket` compiles the pinned original obs-websocket 5.7.5
server, its 52 C++ sources, generated moc/UI inputs and resources. Dependencies
include Qt Widgets/Svg/Network, Asio, nlohmann-json, websocketpp and qrcodegencpp.
Header/library overrides support dependencies outside the system prefixes.
The submodule source and pin are unchanged.

Enable `OBS_BUILD_WEBSOCKET=1` with `OBS_BUILD_APP_GUI=1`. The staged application
loads the server with each core, rejects a wrong password, authenticates a real
client and exercises scenes, inputs, settings, events, batches, errors and a
decoded white source screenshot. With `OBS_BUILD_APP_RECORD=1`, the wire client
starts recording after those checks; the retained H.264/AAC recording is then
decoded and checked. A Qt probe opens Server Settings and Connect Info, checks
that the QR image contains dark and light pixels, and captures both dialogs.

`OBS_BUILD_DISABLE_WEBSOCKET_UNITY=1` compiles the 52 separate C++ sources.
Both modes pass the GUI, wire and recording checks against both cores. Their
11 public `obs_module*` entry points agree; incidental exported C++ template
symbols differ. The core has a separate typed unity option, described below.

The existing `test/websocket-tier4` suite also passes against both staged cores:
193/193 checks and 147/147 documented request types. The external fixture
adapter aliases its private `obs-studio` configuration directory to the renamed
product's `obs-studio-rust` directory; protocol assertions remain unchanged.
The combined websocket, Tools, Lua/Python and NVENC application passes with
17 production modules plus the test-only Qt probe. Settings/Connect Info
screenshots from both cores were visually inspected.

These are functional checks, not controlled build-speed measurements. Browser,
remaining plugins, real capture profiles, Windows/macOS native graphs,
production packaging and configurable unity/PCH across the remaining graph
remain required.

## Shared source inventory and Linux Tier 2

`rust/tools/ported_sources.py` reads `libobs/build/source-sets.json` and expands
the selected unity groups for Tier 2's native-object exclusion checks. CMake
and Cargo share the optional HEVC group in that manifest. The reader matches
the actual CMake inventory exactly: 46 Linux objects with HEVC and 45 without.
The existing minimum inventory size and Ninja graph checks remain in force.
The local validation image includes pinned uv and checks offline system-Python
startup while building the image, matching the repository's Python policy.

This fixes the CI reader that found only one object after the source lists
moved out of CMake. Linux Tier 2 CI passes on `ca331a93d`; local source parity
and HEVC on/off lifecycle/symbol checks also pass (1766/1762 exports
respectively, identical between cores).

## Individual libobs source compilation

`libobs::Options::unity` defaults to true, preserving the selected explicit
groups. Set it to false to compile each implementation separately. The native
engine expands C/C++ BUILD.md groups into generated one-member translation
units, preserving their group headers and keeping the source tree unchanged.
Ordinary and explicitly generated sources retain their original paths.
Wrappers are rewritten only when their contents change. Cargo rerun metadata
tracks the groups, their headers and their implementation members; compiler
includes retain direct dependency on the original contents.

The expander rejects conditional or otherwise unsupported group directives
instead of flattening them into a different source selection. It also rejects
duplicate members and includes outside the source root. Objective-C groups
still require platform implementation.

`OBS_BUILD_DISABLE_CORE_UNITY=1` selects this mode in the native integration
driver. Local checks pass lifecycle/export parity with HEVC on and off against
both cores. With HEVC enabled, all 1766 exports match the retained unity-built
core exactly, and 51 existing C test executables pass against each core with
loader resolution verified. The browser-disabled GUI, websocket wire checks,
white preview/screenshot and real frontend/API recordings also pass against
both cores. These C executables were built by CMake; their build migration is
still required. The direct native fixture also verifies member/header edits
change the loaded result; that is compiler invalidation evidence, not a full
Cargo scheduling experiment.

This establishes the core option. Other targets still need to expose it
consistently, and optional compiled PCH and controlled performance comparisons
remain outstanding.

The subsequent stock-container CI run exposed a separate missing-uv dependency:
unlike the local Docker image, both Tier 2 jobs start from stock Ubuntu. The
existing build and test jobs now provision the same pinned uv before invoking
validation. No workflow files or jobs were added; remote verification remains
required for subsequent conversion commits. This provisioning fix passes the
build and both test modes on `ca331a93d`.

## Building the C tests through Cargo

The current shared inventory is `test/cmocka/build/test-graph.json`, consumed by
both `validation::build_c_tests` and `test/cmocka/native-test-graph.cmake`. The
independent Linux manifest described in the initial results below was retired
in #1805. The graph currently contains 158 targets: 138 core, 12 graphics,
six VLC and two WHIP. This remains a subset of the repository's native tests;
Cargo compilation of these tests currently runs on Linux.

Targets may specify `"cpp_standard": "17"` or `"cpp_standard": "20"`.
The typed native model passes the requested ISO mode to `cc::Build` only for
C++/Objective-C++ compilation. The CMake consumer requires the same standard
and disables C++ extensions, matching the OBS compiler configuration. Omitting
the field retains the existing compiler settings; invalid values fail graph
configuration. A mixed C/C++20 executable contract compiles and runs through
both consumers and checks that C compilation retains its own language mode.

The Qt-free disk-space monitor, scene-collection migration, crash-sentinel
directory, settings migration, output-start logging and unclean-launch tests
now use this model with their original C++17 sources, includes and library
links. Their indirect frontend utility headers are tracked as build inputs.
Existing C/C++ test sources and public headers remain unchanged.

### Initial native test migration (historical)

`validation::build_c_tests` builds the 51 previously selected, unchanged C test
executables through the native engine, using an explicit source/dependency
inventory in `rust/obs-build/manifests/c-tests-linux.json`. It invokes no CMake.
Private implementation files get generated C translation units; test bodies
and public headers remain unchanged. Caption, happy-eyeballs, MbedTLS and
other target dependencies are explicit. Header-only fixture dependencies stay
header-only, including the fake PulseAudio test.

`OBS_BUILD_C_TESTS=1` enables the build and execution checks in the native
integration driver. Each binary runs against both cores from a private working
directory and HOME. Loader resolution must identify the selected core;
stdout/stderr and JSON results are retained. A 60-second limit applies to each
process.

Per-target sanitizers default on for the same four cases selected by the
original CMake configuration. The central platform helper suppresses them if
global compiler flags already select a sanitizer. The two address/undefined
cases retain their frame-pointer and no-recovery options. ELF checks confirm
the expected ASan/UBSan runtime dependencies. Set
`OBS_BUILD_DISABLE_C_TEST_SANITIZERS=1` to select the disabled option.

Local results: 51 compiled executables and 102 successful runs with HEVC and
per-target sanitizers enabled; 50 executables and 100 successful runs with
HEVC/sanitizers disabled and individual-source libobs compilation enabled.
The disabled HEVC target is excluded and disabled sanitizer runtimes are
absent from the selected executable dependencies. This is the selected
51-test inventory, not the repository's complete C test graph; the remaining
targets and their platform/feature conditions still require conversion.

PR CI on `ca331a93d` completed successfully, including the repaired Linux
Tier 2 build and both C/Rust test modes. Run `39078605291` on `4369949a7`
also completed successfully after the VLC and native-test builder changes,
including both Linux test modes and the selected Windows/macOS checks.
These jobs do not automatically exercise the opt-in native application driver.

## Linux VLC media source

`SourceModule::Vlc` builds the original VLC core and POSIX host groups.
It discovers headers through pkg-config, `OBS_VLC_INCLUDE_DIR`, `VLC_PATH`
or standard include directories. Header discovery adds no libVLC link edge:
the original plugin still loads the optional runtime dynamically. The native
check inspects the ELF dependency list to enforce that distinction.

Set `OBS_BUILD_VLC=1` with the staged GUI and websocket checks. The real app
creates a VLC playlist source, decodes a six-second local H.264 clip, returns
a white source screenshot, reports its duration and handles pause/play/restart.
Normal shutdown saves the playlist path in the scene collection. Process maps
must contain the staged plugin and dynamically loaded libVLC.

Both cores pass those checks, frontend recording and the four API recordings.
The combined VLC, Tools, Lua/Python, websocket and NVENC application also passes
with individual-source libobs compilation: 18 production modules plus the
test-only Qt probe. That run uses an explicit include directory with libVLC's
pkg-config lookup disabled, exercising header discovery independently.

`OBS_BUILD_VLC_ABSENT=1` selects the runtime-absence check. In an environment
without libVLC, both cores log that the source is disabled, leave `vlc_source`
unregistered, retain the loaded plugin and exit normally. Process maps must
confirm the runtime is absent; a discovered system installation fails this
fixture rather than weakening the check. This does not establish packaging
of VLC, every codec/playlist mode or actual audio-device output.

### Unchanged VLC compatibility tests through Cargo

`validation::build_vlc_tests` builds all six original Linux VLC cmocka
executables: drain gate, host path, channel order, audio tracks, audio pacing
and playlist gaps. The pacing executable retains its FFmpeg dependencies.
The C sources and their assertions are unchanged. Select the lane with
`OBS_BUILD_VLC=1 OBS_BUILD_VLC_C_TESTS=1` and provide the real libVLC runtime.
It does not require the staged GUI or websocket lane.

The integration driver runs every executable against each core, checks the
selected library with `ldd`, stages the original effects, and isolates each
working directory and temporary/configuration/data/cache directory. It retains
the original 60/120-second limits and rejects skipped tests, including exit 77.
It clears inherited cmocka filters and forces standard output formatting so
external settings cannot silently select fewer cases or hide skipped cases.
Paths are canonicalized before changing the child working directory, including
when `OBS_BUILD_TEST_OUTPUT` is relative. Results and raw logs are retained
under the output directory in `vlc-c-tests.json` and `vlc-c-test-runs/`.

RED `a616f3c57` fails at the missing native test builder. The implemented graph
passes all twelve executions (50 cmocka cases) without skips using libVLC
3.0.23. The combined core/module/render and four MP4/MKV recording validation
took 188.79 seconds. This is runtime compatibility evidence, not a build-speed
benchmark. A local pass does not resolve the playlist audio-integrity failure
seen in prior-head PR CI and tracked by #838.

### Graphics-dependent unchanged C tests

`OBS_BUILD_GRAPHICS_C_TESTS=1` adds the original
`test_group_hide_transition_audio` executable to the Cargo graph. It retains
the original core/caption/cmocka/math dependencies, graphics-module definition,
effects and 120-second timeout. RED `60beca399` fails at the missing builder.
The implemented lane passes all four cases against each core (eight total),
with no skips, in a 40.15-second core/module/render invocation. The native
compiler cache reports 62 hits and two misses for that invocation. This is not
a controlled build benchmark.

The VLC and graphics lanes share the same runner for canonical paths, core
selection, isolated directories, cmocka environment, skip rejection and logs.
The graphics lane writes `graphics-c-tests.json` and
`graphics-c-test-runs/` under the selected output directory.

The combined graphics/VLC run passes all fourteen executable/core combinations
(58 cmocka cases), without skips, plus the lifecycle/render checks and four
validated MP4/MKV recordings. It took 222.04 seconds including runtime checks;
soldr reports 112 cache hits and zero misses. Formatting, all-target/all-feature
Clippy and the primary review pass. This validates the shared runner extraction.

The prior-head CI run `39091137249` fails this executable against the Rust
core: all four cases report no program audio at line 216. Its log shows audio
buffering growing from zero to the 960 ms maximum before the failures. The
C-core Linux lane and Windows/macOS Tier 2 lanes pass. The local pass above
does not resolve that failure.

An isolated diagnostic of the original `add_audio_buffering` function found
that a positive delay below one sample rounds to zero frames/ticks, leaving
`new_ts.end` uninitialized before copying it to the caller. With GCC's pattern
initialization enabled, verbatim function extraction reproduces invalid
intervals for 1, 10000 and 20833 ns delays at 48 kHz. A second probe directly
includes the original source, uses the real headers and links the native core
with no dependency stubs; it reproduces all three invalid intervals and the
valid one-frame case. A scratch copy with the local interval initialized from
the incoming interval corrects all diagnostic cases while retaining the
one-frame case, with the same real headers/core and compiler flags. This
establishes a function-level defect, not causation of the full CI failure.
No production audio source or existing C test is changed at this checkpoint.
Reproduction source and results are retained in the local reports
directory under `audio-buffering-probe/` for follow-up.

### Audio interval regression and scoped fix

Draft [#1385](https://github.com/obs-rust/obs-studio/pull/1385) carries the
follow-up to that diagnostic, and this branch carries the same change.
RED `a720d011f` adds a new C regression that directly includes the original
private audio implementation. Two of its five cases fail in each linked-core
mode: empty and pending buffers with a sub-sample delay. The positive one/two
tick and full-buffer controls pass. GREEN `52c577638` initializes the local
interval from the incoming interval; all five cases then pass in both modes.
Existing C tests and public headers remain unchanged. The CMake fragment
configures, builds and passes CTest locally with real imported core/caption
artifacts and headers, without dependency stubs.

The Cargo test manifest adds the same regression. RED `d6388ca86` requires the
missing executable and observes 51 instead of 52. The manifest supports
compiler-checked optional flags and explicit linker options, preserving the
test's optional pattern initialization and section garbage collection.
The core battery now has 52 executables (51 with HEVC disabled). A full enabled
run passes all 104 executable/core combinations, with 832 reported cmocka cases
and no skip markers; the two standalone video-statistics executables do not
emit cmocka summaries. The new regression contributes five cases per mode.

This direct-include regression checks the original C algorithm even when it
links the Rust core. It does not establish a Rust replacement for that helper,
nor that the entire intermittent CI failure in #838 is resolved. The unchanged
runtime tests and full CI remain necessary.

With the source fix, the combined core battery, unchanged group-audio test and
browser-enabled OBS GUI pass against both cores. The app verifies actual
browser pixels/JavaScript, websocket behavior, saved configuration, frontend
H.264/AAC recording, rendering and clean OBS shutdown; the API fixture verifies
four additional MP4/MKV recordings. Runtime directories are
`app-runtime-VCOfty` and `app-runtime-BwMSdi`. The whole invocation takes
489.49 seconds, with 164 compiler-cache hits and 165 misses; it includes native
compilation and runtime tests and is not a controlled build benchmark.

A subsequent run of all six unchanged VLC executables against both fixed cores
passes all 50 cases without skips: 180.83 seconds including runtime checks,
52 cache hits and eight misses. Formatting, Clippy and primary review pass.
The prior parent-head CI run `39092443140` (`b547ea121`) fails the unchanged
recording-shutdown wait test in mode OFF (one failing executable out of 296,
lines 121/142, tracked by #1283); three PulseAudio tests are skipped in that CI
environment. This audio interval fix does not claim to fix that separate wait
race or to explain all intermittent audio failures.

The separate fix PR CI run `39093013032` also finishes with a failed gate:
mode ON passes, while mode OFF fails the same two recording-shutdown wait
cases at lines 121/142 (one executable out of 299). The interval regression
passes. This is not a green CI result and no retry was used to hide it.

## Optional compiled PCH for Linux cores

`libobs::Options::pch` defaults to false and is independent of `unity`.
Set `OBS_BUILD_PCH=1` in the native integration driver to compile the explicit
group headers with GCC or Clang on Linux. This currently covers libobs;
other targets and Windows/macOS PCH remain required work.

The native engine accepts explicit `PrecompiledHeader` source assignments.
It compiles each header through the same `cc::Tool` and flags as its consumers,
keeping C and C++ settings separate. Artifact directories distinguish host,
target, header path, language, compiler command and environment. Compiler
invocations retain the soldr wrapper. GCC rejects invalid PCH and uses an error
header to detect fallback; Clang consumes an explicit PCH artifact. Requested
PCH therefore cannot silently fall back to parsing a normal header.

Compilation batches preserve source order, including interleaved assignments
to different headers. A regression fixture caught constructor order changing
from 123 to 213 when assignments were initially sorted; ordered batches pass
with unity enabled and disabled. Mixed C/C++ fixtures also verify direct and
transitive header edits and language-specific flag changes affect the loaded
result. GCC 15.2.0 passes
all nine native contracts; Clang 21.1.8 passes both PCH contracts.
The real Clang core pair with unity and PCH enabled also passes the lifecycle,
public data, module registration and exact C/Rust export comparisons. The
Clang run does not include the C-test executable or GUI matrices.

The real GCC core pair passes export/lifecycle checks and all 102 selected
C-test runs with PCH enabled, with unity enabled and disabled. PCH artifacts
are currently requested on every direct native build; repeated invocations
depend on compiler-cache behavior. These checks establish correctness for the
tested configurations, not a speedup or complete Cargo invalidation coverage.

The combined browser-disabled Linux GUI passes against both individual-source,
PCH-built cores with VLC, frontend Tools, Lua/Python, websocket and NVENC enabled.
It exercises the saved white scene, VLC playback controls, dialog and wire
checks, frontend hardware recordings and four API recordings, then normal
shutdown. The complete build/validation invocation took 571.94 seconds; this
includes native compilation and runtime checks and is not a build benchmark.

### Scoped core build measurements

Corrected local GCC measurements explicitly set `NUM_JOBS=4` for the standalone
native integration process and `CARGO_BUILD_JOBS=4`. The active process
environment was inspected: both values were 4 and no Cargo jobserver was
inherited. The earlier wrapper only set Cargo's limit, so its claim of four
native jobs is withdrawn; its original observations are retained separately.

The corrected matrix uses fixed opt-level 2 with debug information, one fresh
output directory per mode and three repeated builds. The interval covers the
C/Rust core pair and caption, excluding plugin builds, GUI checks and Cargo
startup. The host and compiler cache were shared; no cache was reset. Fresh
output is therefore not a cold-cache measurement. Trials ran in table order.
The integration driver writes `core-build-timing.json` under
`OBS_BUILD_TEST_OUTPUT`; repeat that invocation with the same directory for
additional samples.

| Unity | Compiled PCH | Fresh output | Repeat median | Repeat range |
| --- | --- | ---: | ---: | ---: |
| On | Off | 12.60 s | 2.55 s | 2.50–2.94 s |
| On | On | 38.39 s | 5.72 s | 4.70–8.67 s |
| Off | Off | 26.38 s | 5.82 s | 5.30–6.34 s |
| Off | On | 44.18 s | 9.83 s | 9.07–11.65 s |

All twelve repeat invocations reported zero compiler-cache misses. Cache
summaries cover the whole integration invocation, not just the timed core pair.
These results support unity on / PCH off as the default for this measured core
case. PCH preparation and per-header compilation batches still need performance
work. This does not establish full application build time, clean-cache
performance, a CMake comparison or Cargo no-op behavior.

### Audit of duplicate PCH generation

At the user's request, the GCC Linux core-pair build was audited with temporary
request tracing and forwarding C/C++ compiler wrappers. Both audited invocations
passed the real core integration test. The first invocation used the audit
wrappers' compiler identity; the next reused the same artifacts and flags.

| Invocation | PCH requests | Distinct PCH artifacts | Compiler executions |
| --- | ---: | ---: | ---: |
| Initial audited build | 18 | 14 | 14 |
| Warm repeat | 18 | 14 | 0 |

Some headers recur in nonadjacent source batches and across the common/C/Rust
variant stages. There are four redundant compiler-cache requests per core-pair
build, but the trace did not find duplicate compiler generation: each initial
artifact was compiled once, and soldr served every warm request from cache
(the whole warm invocation reported 64 hits and zero misses).

The PCH path signature selects an artifact namespace; it is not a freshness
check. Generation must not be skipped merely because that file exists: direct
headers, transitive headers, flags and compiler identity still require correct
dependency validation. An optimization can deduplicate requests within an
immutable build invocation while preserving source/constructor order. Persistent
reuse needs a separate validity check. Temporary tracing was removed after the
audit. The earlier PCH timings describe this implementation, including cache
request and batch overhead; they do not establish an intrinsic PCH speed limit.

## Bundled obs-sys configuration and Cargo scheduling

The `bundled` feature exposes the core build modes through environment variables:

Bundled native optimization and debug symbols follow Cargo's effective
`OPT_LEVEL` (`0`, `1`, `2`, `3`, `s`, `z`) and `DEBUG` (`true` or `false`),
including custom profiles. Cargo's `PROFILE` name is not used. Set
`OBS_SYS_NATIVE_PROFILE` to `debug`, `release`, `relwithdebinfo` or `minsizerel`
to select the corresponding standalone native policy explicitly.

Native assertions default on at optimization level `0` and off otherwise.
`OBS_SYS_NATIVE_ASSERTIONS=0|1` overrides this independently of debug symbols
and named native profiles. This is a native policy: Cargo does not expose a
reliable target debug-assertion setting to build scripts, so custom Rust
assertion settings are not discovered. Swift receives the same native
assertion policy explicitly. All four configuration inputs are tracked for
Cargo rebuilds; invalid values fail before compilation.

| Variable | Values | Default |
| --- | --- | --- |
| `OBS_SYS_UNITY` | `0` or `1` | `1` |
| `OBS_SYS_PCH` | `0` or `1` | `0` |

The options are independent. Empty, non-text or other values fail configuration
before native compilation. PCH currently requires Linux GCC or Clang, as above.
With the native dependencies installed, for example:

```sh
OBS_SYS_UNITY=0 OBS_SYS_PCH=1 soldr cargo test -p obs-studio-rust --features bundled-native --test obs-tests-app
```

The build script tracks both variables and passes them to the shared libobs
builder. Four bundled contracts cover parsing/validation, requested versus
compiled settings, the public API version and native startup/shutdown. Tests
read the inherited environment without mutating process-global settings.

A local fifteen-invocation Cargo experiment used a separate target directory,
four compilation jobs and the shared host/compiler cache. All four mode
combinations passed. Changing either option made Cargo mark `obs-sys` dirty
for that variable. An invalid PCH value failed the build script before changing
the native library. Five no-change invocations reported the package fresh and
kept the native library's nanosecond timestamp unchanged, including PCH modes.

The experiment then made successive temporary edits to three private inputs,
changing one additional file per step with unity and PCH enabled. Each change
made Cargo rerun the build script.
A separate process loaded the resulting library and asserted its changed
public API return value or version string:

| Input | Observable change | Invocation time | Reported cache hits/misses |
| --- | --- | ---: | ---: |
| `libobs/obs.c.h` | API version increment | 14.15 s | 52 / 1 |
| `libobs/build/libobs.pch.h` | Further API version increment | 19.54 s | 47 / 6 |
| `libobs/obsversion.c.in` | Generated version string suffix | 11.07 s | 52 / 1 |

Times and cache summaries cover the whole Cargo/test invocation, not just
native compilation. These are observed incremental cases on a shared host,
not isolated benchmarks. All temporary inputs were restored; a final build
returned the original API values and its subsequent no-change run stayed fresh.
This verifies selected core input scheduling, not every generated frontend,
plugin, SDK or packaging dependency. The full conversion remains incomplete.

PR CI run `39080352730` on `5f04b9f0a` completed successfully after the PCH
checkpoint, including both Linux core test modes and the selected Windows/macOS
lanes. Run `39081209344` on `352bac56d` also completed successfully for the
bundled-mode changes, including both Linux modes and selected Windows/macOS lanes.

## Linux browser build

`plugins::build_browser` builds the pinned `obs-browser` module and its
`obs-browser-page` subprocess helper. It uses the existing CEF SDK's shared
library and wrapper archive; it does not build Chromium. The SDK must provide
`include/`, `Release/`, `Resources/` and the prebuilt CEF wrapper archive.
`BrowserOptions` selects the SDK directory, Qt panels and unity compilation.
The Linux implementation requires CEF >=95, nlohmann_json >=3.11, X11 and libdrm.
The pinned browser source unconditionally includes `QApplication` and uses
`QToolTip`, so Qt Widgets headers and libraries are required even with panels
disabled. That mode omits panel sources and exports. Enabled panels use the
original Qt sources and generated MOCs.

Browser unity compilation uses three explicit, ordered groups to avoid duplicate
Base64 declarations and default arguments in the untouched submodule sources.
Individual compilation uses the same source inventory. The helper has its own
source list and CEF dependencies. `BrowserArtifacts::deploy_runtime` copies the
CEF Release and Resources trees and the helper beside the browser module,
including locales, data packs and auxiliary shared libraries. These SDK resource
directories are Cargo build inputs.

`frontend::build_application_with_options` enables the original custom-browser
dock implementation and generated Qt interfaces through `browser_panels`.
The existing `build_application` entry point retains its default configuration.
For the integration driver, set `OBS_BUILD_BROWSER=1` and `OBS_CEF_ROOT` to the
SDK root. `OBS_BUILD_DISABLE_BROWSER_UNITY=1` selects individual browser
compilation; `OBS_BUILD_DISABLE_BROWSER_PANELS=1` disables the panel interfaces.
The GUI browser tests also require `OBS_BUILD_WEBSOCKET=1` to exercise the
JavaScript bridge through the public source and scene APIs.

The SDK used locally is the Linux x86_64 CEF 7871 archive pinned by
`CMakePresets.json`, with SHA256
`8ffc64d06ea74da613eeda7f2aa5d4e98661af6fa3ed8c68724c9a6e4e89ea8e`.
Its CEF header reports 150.0.19. Host verification supplies installed CEF runtime
dependencies through a private loader path; it does not modify system libraries.
This browser implementation currently targets Linux. Other platforms, production
SDK acquisition, installation and packaged runtime validation remain unfinished.

Browser validation is in progress. The module/helper build contracts pass with
unity and panels independently enabled or disabled, including helper execution,
CEF linkage and panel export selection. The panels-enabled unity and individual
modules each export the same 22 symbols. In the staged GUI, the original custom
dock editor accepts real delegate edits, creates a browser dock, navigates to a
local page and receives its JavaScript-generated readiness title.

During development, the full browser GUI test failed at the browser source's
local-file navigation. The input is active and showing at the requested 128×72 dimensions,
but its screenshots remain transparent. A diagnostic CEF DevTools session finds
the requested `http://absolute/…` target with an `about:blank` page context; an
owned-process trace observes the initial paint but no call to the local-file
scheme handler. Individual compilation reproduces this failure. An additional
diagnostic run caught `CefTask_0_CToCpp called with invalid version -1` during
cold panel initialization. The dependency fix and isolated credential-store
fixture correction below resolve these observed failures in the tested Linux
runtime; this does not establish all-platform or packaged application parity. The new checks
retain their rendering, JavaScript bridge, helper-process and saved-profile
requirements. Existing C tests and public headers remain unchanged.

### Browser startup investigation

A CMake reference build of the original browser module and helper reproduces
the local-file failure inside the same Cargo-built frontend and core. This
rules out browser unity compilation as its cause; it does not establish what
a complete CMake-built application does. Explicitly selecting the global CEF
request context did not fix it and was discarded.

The cold-start API-version abort has a separate focused regression in a sister
`obs-browser` checkout. Linking the real browser objects and queuing a task
before initialization aborts with the original module and safely rejects the
task with the proposed fix. Real module startup followed by immediate unload
also aborts with the original module and exits successfully with the fix.
The dependency draft [obs-browser#1](https://github.com/obs-rust/obs-browser/pull/1)
proposes commit `0c3b700` (the current experimental pin). It waits for context readiness for ordinary tasks,
handles source destruction before readiness, and coordinates unload with
initialization success or failure. A follow-up dependency test at `4f26bda`
now exercises real CEF initialization failure: a second process shares the
first process's locked profile, waits for the actual initialization-failure
message, then unloads cleanly. Both the original and fixed modules pass this
failure-path characterization; it does not distinguish the startup-race fix.
The parent's production pin is now `7bab463` (obs-browser 2.27.3, which includes #1 and `4f26bda`). Browser-source rendering is
verified in the disposable Linux test profile described below.

### Isolated browser credential store

A CEF network trace narrows the blank local source to an unfinished global
persistent-cookie key load in the isolated desktop session. The custom panel's
separate cookie store completes. Selecting Chromium's basic password store
inside a fresh disposable test profile completes the global cookie load and
passes the existing browser source color/JavaScript callback, WebSocket,
recording, saved-scene and shutdown checks. A second runtime-only run with the
basic store and no network tracing also passes. This diagnoses the isolated
credential-service integration; it does not verify normal desktop keyring
integration. No production credential-store option is changed. Full native
Cargo GUI validation now passes against both C and Rust core variants, including
the original dock editor, actual native dock pixels, local source HTML/JavaScript
callback, staged helper subprocesses, saved configuration, recordings and normal
shutdown. The full invocation took 510.62 s and reported 27 cache hits / 199
misses; it used the earlier uncapped native test environment and is not a
controlled build benchmark. A warm repeat with both job limits explicitly set
to 4 passes in 77.56 s, with 226 cache hits and zero misses. Its interval includes
GUI checks and recordings against both cores; it is not a compile-only timing.


## Optional FDK AAC encoder checkpoint

`plugins::build_fdk` builds the original `obs-libfdk` unity group as a dynamically
loaded module through `cc::Build`, discovering the `fdk-aac` SDK through
pkg-config. This optional encoder remains disabled by default, matching the
original CMake option. Set `OBS_BUILD_FDK=1` in the native integration driver to
build, load and exercise it. GUI selection alone does not enable FDK.

The RED commit `a0bc15642` fails at the unimplemented native builder. With the
builder implemented, the integration test passes against both the C and Rust
cores in 24.77 s. Each core records MP4 and MKV with the actual `libfdk_aac`
encoder; ffprobe verifies AAC LC, 48 kHz stereo, decoded audio frames, H.264 video
frames and recording duration. No usable soldr statistics were available for
that invocation, so it does not establish cache performance. The existing
headless path with FDK disabled also passes (12.80 s), using `ffmpeg_aac` for
all four recordings.

A reference build invokes the original `plugins/obs-libfdk/CMakeLists.txt`,
using the same FDK 2.0.3 SDK and Cargo-built core. Both modules export exactly
the same six dynamic symbols. This verifies module exports; it is not a
complete CMake application or a cross-platform runtime comparison. The default
GUI/browser path is checked separately to ensure adding this optional encoder
does not make it mandatory.


## Optional JACK and sndio audio modules

`SourceModule::Jack` and `SourceModule::Sndio` build the original Linux unity
groups as dynamic modules through `cc::Build`, with pkg-config dependencies
`jack` and `sndio`. Both remain disabled by default, including ordinary GUI
runs. Select them independently with `OBS_BUILD_JACK=1` and
`OBS_BUILD_SNDIO=1` in the integration driver. The GUI core-module whitelist
includes each only when explicitly selected. The sndio target retains the
original `-Wno-error=vla` option.

RED `452911b5c` fails at the unimplemented optional audio module graph. With
that graph implemented, both modules load and register their original
`jack_output_capture` and `sndio_output_capture` input IDs against both C and
Rust cores; the existing graphics and four-recording checks also pass (20.52 s).
Reference builds invoke each original CMake target with the same SDKs (JACK
1.9.22 and sndio 1.10.0). Each module exports exactly the same seven symbols as
its Cargo-built counterpart. The JACK reference needs an explicit include path
because its original finder's pkg-config include hint uses the singular name.
No CMake source list was recreated for these reference builds.

### Actual input capture

The new C fixture uses unchanged public OBS headers and real audio clients. It
sends distinct stereo samples (left 0.25, right -0.125) through each original
source, requiring at least three matching packets at the public source audio
capture callback. Event waits are bounded; JACK clients close, the sndio writer
thread joins, callbacks are removed and sources are released before the local
probe state is destroyed. No existing upstream C test or public header changes.

- JACK: start a separately named non-realtime `jackd` dummy server, export its
  `JACK_DEFAULT_SERVER`, and set `OBS_BUILD_JACK=1 OBS_BUILD_JACK_CAPTURE=1`.
  The fixture connects its writer to the real OBS source's two JACK ports.
- sndio: start an owned `sndiod` with playback/monitor mode (`-m play,mon`) and
  a named sub-device, set `OBS_BUILD_SNDIO=1`, and pass its descriptor as
  `OBS_BUILD_SNDIO_CAPTURE_DEVICE` (for example `snd,11/capture` for server
  unit 11 and sub-device `capture`). The fixture writes through the real sndio
  client API and observes the monitor source. Use a clocked audio backend.
- Either capture flag requests the existing graphics probe and initializes
  stereo audio; recording is optional. The existing probe still requires an
  X11 display and an OpenGL implementation. These are Linux integration tests.

A direct ALSA null backend did not drive sndiod: its watchdog timed out and the
sample test failed. Keeping the assertions intact, the validated fixture routes
sndiod through ALSA's PulseAudio plugin into a private PulseAudio null sink.
No desktop audio service or physical input is used. The combined run passes
against both cores, exercises both input captures in each of the four recording
processes, and validates all recordings (33.67 s, 96 cache hits / zero misses).
That interval includes runtime capture and recordings; it is not a compile-only
benchmark. Capture-only validation with recording disabled also passes against
both cores (15.53 s), exercising both inputs without relying on recording to
initialize audio. Real physical-device routing, platform graphs and packaged audio
runtime remain separate requirements of the full conversion.


Independent JACK-only and sndio-only configurations also pass. The staged
GUI/browser invocation with both optional modules selected passes against both
cores, including native dock/source rendering, JavaScript/WebSocket checks,
recordings, saved configuration and shutdown (102.18 s, 226 cache hits / two
misses). This is a combined build/runtime interval, not a controlled compiler
benchmark or proof of physical-device capture.


## Intel QSV native module

`plugins::build_qsv` builds the original Linux x86_64 QSV C/C++ groups in CMake source
order, using oneVPL >= 2.9, libva and libva-drm. It retains the host-definition
header, language standards, switch warning exception and inline visibility.
`QsvOptions` supports HEVC, unity and compiled PCH independently; its default is
HEVC on, unity on, PCH off. The integration driver selects the target with
`OBS_BUILD_QSV=1`, with `OBS_BUILD_DISABLE_QSV_UNITY` and `OBS_BUILD_QSV_PCH`
controlling its compilation. Its HEVC setting follows `OBS_BUILD_DISABLE_HEVC`.
The full CMake graph enables QSV by default; the integration driver's opt-in is
not a change to that product default.

RED `3f0a49d4f` fails at the missing builder; implementation `5c511b9d2` adds the
native graph and isolated CMake runtime comparison. The initial build/load and
four H.264/AAC recordings pass against both cores (34.56 s). The controlled
reference uses each original QSV CMake target, matching the project's inline
visibility and optimization, against the same SDKs (oneVPL 2.16 and libva 2.23).
The finder needs the pkg-config include root explicitly on this Nix host.
`OBS_BUILD_QSV_REFERENCE` requires exact dynamic exports and compares encoder
registrations after loading that actual reference module with each core in a
fresh process. Reference runs preserve the real graphics/module/lifecycle
checks; their recording is omitted to avoid overwriting the validated clips.

The first unity/PCH-off case passes with seven matching exports and matching
registrations. Both cores and the CMake reference expose no QSV encoders on this
host. This proves graceful loading without a detected implementation; Intel
hardware encoding and texture interoperability remain unverified. The complete option matrix passes, with seven matching exports and four fresh
CMake reference runtime comparisons per row:

| QSV unity | QSV PCH | HEVC | Full invocation | Cache hits / misses |
| --- | --- | --- | ---: | ---: |
| On | Off | On | 25.46 s | 96 / 1 |
| Off | Off | Off | 25.98 s | 97 / 1 |
| On | On | On | 30.56 s | 93 / 8 |
| Off | On | Off | 29.90 s | 91 / 12 |

Each invocation includes both cores, native compilation, real MP4/MKV recordings
and reference runtime checks. These are functional validation intervals with a
shared cache, not controlled PCH performance measurements. Disabled rows also
check the public MP4/MOV codec lists. Compiled PCH remains off by default.

### HEVC-disabled MP4 build regression

The QSV matrix's HEVC-disabled recording case exposed unconditional HEVC calls
in the existing MP4 muxer. Both Cargo and the original CMake output target fail
to link against the disabled core, with unresolved `obs_parse_hevc_header` and
`obs_parse_hevc_packet`. The original CMake reference uses its actual target,
SDK libraries and shared dependencies, and `--no-undefined` to require a valid
module. The failure is not QSV-specific.

RED `0c73e4776` additionally requires MP4/MOV advertised codecs to agree with the
build option. The fix gates the HEVC-only helpers, call sites and recognition
with `ENABLE_HEVC`, and removes HEVC from the disabled outputs' codec lists,
following the existing FLV output pattern. Enabled source bodies remain intact.
The original CMake output target now links successfully with HEVC disabled.
The next disabled recording run exposes one VA-API codec switch case calling
`vaapi_device_hevc_supported` despite that helper's declaration and definition
being conditional. Guarding the case with the same feature macro makes it use
the existing unsupported-codec fallback when disabled. Enabled behavior stays
unchanged. These regressions were found by keeping real recordings in the
HEVC-disabled option matrix.
No public header or existing upstream C test is edited. Full application,
physical Intel hardware, cross-platform and release packaging parity remain
requirements of the complete conversion.


The full staged GUI/browser invocation with global HEVC disabled and QSV
selected passes against both cores (424.83 s, 110 cache hits / 118 misses).
It verifies native dock/source pixels, JavaScript/WebSocket behavior, saved
configuration, frontend recording and normal shutdown. With
`OBS_BUILD_MOV_RECORD=1` (requiring recording/FFmpeg targets), the API fixture
also records and decodes MP4, MKV and MOV against each core. Both MOV artifacts
contain H.264/AAC and the QuickTime `qt  ` major brand. Six additional CMake QSV
reference runtime comparisons pass. This interval includes native compilation
and application checks; it is not a compile-only or controlled cache benchmark.
Intel hardware encoding remains unverified.


## Linux WebRTC / WHIP Cargo graph

RED `2923db820` requires the real WebRTC output and service module and fails
at the missing builder. `plugins::build_webrtc` now compiles the original
POSIX and core C++ groups in their original order through `cc::Build`, links
to the selected shared libobs, libdatachannel SDK and libcurl, and retains the
original module data. Unity defaults on and compiled PCH defaults off; both
can be selected independently through `WebRtcOptions`. Its HEVC setting
follows the global option. The integration fixture checks WHIP's advertised
video/audio codecs as well as the output/service registrations.

The explicit `DataChannelSdk` supplies include/library paths and the media
and websocket interface definitions from that SDK's build. Media support is
required and the headers must report libdatachannel >= 0.20, matching the
original dependency requirement. The tested SDK is the shared build of
libdatachannel version 0.24.1 with both interface features enabled. Linux is
implemented; other platforms return an explicit unsupported-graph error.
Static SDK linkage and other SDK feature combinations have not been validated.

Enable the existing opt-in integration driver with `OBS_BUILD_WEBRTC=1`,
`OBS_DATACHANNEL_INCLUDE_DIR` and `OBS_DATACHANNEL_LIBRARY`.
`OBS_DATACHANNEL_DISABLE_WEBSOCKET` describes an SDK built without that feature;
it must match the supplied SDK. `OBS_BUILD_DISABLE_WEBRTC_UNITY` and
`OBS_BUILD_WEBRTC_PCH` select the module's build modes.

The new `tests/libobs/whip_receiver.py` wraps the native test command and
provides a real localhost WHIP receiver using pinned aiortc/aiohttp/numpy.
It authenticates the SDP POST and resource DELETE, negotiates a real peer,
and decodes received media. The native fixture uses the public OBS service,
output and encoder APIs, x264 video and FFmpeg Opus audio; no transport stub
or mocked encoder is involved. Each core sends a rendered 64x64 scene and a
48 kHz stereo 1 kHz tone. The receiver checks decoded pixels, advancing
media timestamps, audio format, tone frequency/RMS, connected state and
resource deletion. The existing four MP4/MKV recording assertions remain.
The wrapper propagates command failures and bounds its owned process group
with a configurable timeout; it does not stop shared services.

The first stream run passes against both cores (20.71 seconds, 96 compiler
cache hits and zero misses). An actual reference module built using the
original `plugins/obs-webrtc/CMakeLists.txt` has the same 26 dynamic exports
as the native module, including seven public OBS module entry points.
Both link dynamically to libobs, libdatachannel and libcurl. The integration
driver compares those exports when `OBS_BUILD_WEBRTC_REFERENCE` is supplied;
`OBS_BUILD_WEBRTC_USE_REFERENCE` explicitly selects that module for a separate
reference runtime run. That flag requires a reference path and never counts
as native-module streaming coverage.

This checkpoint validates H.264/Opus streaming on Linux. Actual HEVC/AV1
streaming, simulcast, external services, Windows/macOS and release packaging
remain required work; codec registration alone does not prove those paths.

The four unity/PCH combinations each pass two native-module WHIP streams and
four MP4/MKV recordings. The separate CMake reference run passes the same
receiver and recording checks against both cores:

| WebRTC unity | WebRTC compiled PCH | Module used at runtime | Native invocation | Cache hits / misses |
| --- | --- | --- | ---: | ---: |
| On | Off | Cargo | 56.62 s | 12 / 84 |
| Off | Off | Cargo | 25.73 s | 94 / 4 |
| On | On | Cargo | 29.10 s | 93 / 4 |
| Off | On | Cargo | 25.17 s | 96 / 4 |
| On | Off | CMake reference | 19.52 s | 95 / 0 |

These are complete native test invocations including runtime/recording checks,
using a shared host/cache in table order. They do not measure isolated compile
time, a cold cache or an intrinsic PCH performance difference. Each native
mode compares its complete export set with the original CMake module before
runtime; the reference row explicitly substitutes that original module.


With global HEVC disabled, a further native-module run passes the advertised
`h264;av1` video/`opus` audio codec checks, two real H.264/Opus streams and four
recordings against both cores (55.70 seconds, 41 cache hits / 54 misses).
This does not exercise actual HEVC or AV1 media. The original WHIP service's
own codec list remains unchanged; it advertises HEVC independently of the
output option, which is existing behavior rather than a newly fixed path.

### Existing standalone WHIP tests

A focused test in the existing opt-in Cargo target `libobs` builds the unchanged
`test_whip_rtp_timestamp.c` and `test_whip_host_os_name.cpp` sources using
`validation::build_webrtc_tests`. RED `a425f96d8` fails at the missing builder;
the implemented graph passes all six RTP-clock cases and both POSIX host-name
cases, with no skips, in 1.27 seconds. These helpers require neither libobs
nor libdatachannel. They run once, not once per core, and do not establish
coverage of a Rust replacement. C and C++ language options are separate,
matching the original test source languages. Each executable has a bounded
60-second runtime; cmocka filters are removed and explicit case totals are
checked. The helper graph currently supports Linux.

```sh
soldr cargo test -p obs-studio-rust --features libobs-integration --test obs-tests-app -- native_libobs::webrtc_tests::original_whip_clock_and_host_tests_pass_without_a_core_or_sdk --exact --nocapture
```

The selected native graph now contains 52 core executables, six VLC tests,
one graphics test and two standalone WHIP tests. It is not the complete OBS
test battery. A static inventory of this worktree's `test/cmocka/tests.d`
finds 238 literal executable declarations before evaluating platform/option
conditions; recent reference CI runs contain 299 configured tests. That
inventory is a gap-finding aid, not an evaluated replacement for CTest.


The consolidated helper test passes all eight cases again in 0.69 seconds,
with one unrelated integration test filtered out. The workspace remains at
its existing cap of 25 Cargo integration binaries; no cap increase is needed.

A broader app/VLC/browser invocation exposed a harness error after 480.98
seconds: it tried to copy its own integration executable only after lengthy
native compilation, and that path no longer existed following concurrent
Cargo rebuilds. All 104 core executable runs had completed before this
failure; no GUI or WHIP-stream pass is claimed from that invocation. The
fixture now snapshots its executable at parent startup, before native builds,
and deploys FFmpeg/NVENC helpers beside that snapshot later. Review and Clippy
pass for this staging change. The failed log is retained, and the full runtime
battery is rerun with no overlapping Cargo commands in this worktree.


The next broad invocation reaches renderer setup, then catches a second
fixture bug: the WHIP guard required `OBS_BUILD_RECORD` to be explicitly set,
while the parent enables recording implicitly for `OBS_BUILD_APP_GUI`.
The guard now uses the parent's existing `enabled` helper, preserving the
same recording requirement for both entry paths. That failed invocation
passes all 104 core executions and both standalone WHIP executables first;
it does not reach GUI, VLC/group-audio or streaming validation.

At the user's request, the remaining conversion and broad validation are
being deferred. A final focused app/streaming attempt is interrupted after
the shared Soldr daemon loses its control connection; final Clippy also
reports a daemon connection reset rather than a source diagnostic. The
previous completed Clippy/review runs and streaming matrix remain valid for
the code they exercised. The final guard change is reviewed locally, but a
successful final-head app run and lint rerun remain outstanding. No shared
broker or daemon is restarted. Keep the PR draft.

Deferred work retains the original scope: remaining plugin graphs and full
native test coverage; Windows/macOS graphs and runtime; physical capture and
hardware codec paths; SDK acquisition, standalone obs-sys publishing and
release installation/packaging; graph-wide unity/PCH controls and controlled
full-app performance measurements. This checkpoint is not the completed
CMake replacement and must not be merged as one.
