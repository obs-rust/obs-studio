# Cargo-driven OBS native build conversion

Tracking issue: <https://github.com/obs-rust/obs-studio/issues/1332>.

The requested final state is a working OBS Rust application built through Cargo
and `cc::Build`, including shared libraries, plugins, generated files, graphics,
Qt, resources, tests and platform packaging. A core-only archive or test fixture
does not complete the conversion.

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
reference and `libobs-with-rust` into separate directories with the same SONAME.
The Rust archive is whole-archive linked and its internal symbols are hidden by
the existing export map. The twin is never installed.

Build the release archive with `soldr cargo build -p libobs-rust --release`.
With native dependencies available, set `OBS_BUILD_RUST_ARCHIVE` to the absolute
`target/release/liblibobs_rust.a` path and run
`soldr cargo test -p obs-build --features libobs-integration --test libobs`.
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
unchanged output contents/mtime. `OBS_QT_MOC` selects a host tool explicitly;
default discovery uses Qt6Core's pkg-config libexec directory. Application builds
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

## Linux frontend executable checkpoint

`frontend::build_application` compiles and links the frontend executable using an
explicit source inventory, shared Qt interfaces, an Idian archive, JSON11, BPM,
libcaption, the frontend API and a caller-provided Rust GPU status archive. It
creates UI/plugin-path configuration in the output tree. This initial frontend
configuration is Linux with browser panels disabled; other feature/platform
configurations and release paths remain under conversion.

Build the GPU archive with
`soldr cargo build -p obs-gpu-status-frontend --release`, then enable the integration
check with `OBS_BUILD_APP=1` and `OBS_BUILD_GPU_ARCHIVE` pointing to the resulting
`target/release/libobs_gpu_status_frontend.a`. The executable runs and reports
`OBS-Studio-Rust 0.1.0 (based on OBS 33.0.0-cc-build)` through `--version`.
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
