# AppImage

`.github/workflows/appimage.yaml` builds OBS-Studio-Rust as an
[AppImage](https://appimage.org) and starts it in stock Debian, Ubuntu, Fedora and
Arch Linux containers to prove that it runs on other distributions and that its
captures work (see Tests). A push to master publishes it as the `continuous`
pre-release, a tag push attaches it to that tag's release.

| File | Purpose |
| --- | --- |
| `assemble-appdir.sh` | Turns a `cmake --install` tree into an AppDir: bundles Qt plugins and every shared library (including all of glibc and its loader), then calls `set-runpaths.sh`. |
| `set-runpaths.sh` | Gives every bundled ELF file a relative `DT_RPATH` pointing at `usr/lib`. |
| `build_vaapi_compat.py` | Builds the pinned Ubuntu libavutil source with optional newer VA mapping support, then checks exported ABI and registered hardware backends against the original library. |
| `AppRun` | Entry point. Installs the bundled loader, sets up the environment, runs OBS and writes a crash report if it crashes. |
| `check-appdir.sh` | Fails when a path `AppRun` exports into the AppDir is missing or empty, or when PipeWire's `client.conf`, the modules and SPA plugins it loads, gconv or the Qt xcb plugin are not bundled. Run by `assemble-appdir.sh` (fails the build) and again by the test. |
| `test-appimage.sh` | The distribution test, run inside a stock container (see below). |
| `testkit.c` | Test helper built next to the AppImage: runtime probes, a fake ScreenCast/Camera portal, the test tone. The test runs it with the AppImage's own loader and libraries. |
| `gpu-test.c` | Physical render-node checks: GBM buffer export and EGL DMA-BUF import with pixel readback, and VAAPI decoding with downloaded frame data. Built as a separate test artifact. |
| `verify-recording.sh` | Checks the recording the test made (outside the container, which must not have ffmpeg). |

## How it works

- **Everything is bundled** except the GPU driver stack, which has to come from
  the host: `libGL`, `libGLX`, `libEGL`, `libOpenGL`, `libGLdispatch`, `libgbm`,
  `libdrm`, `libva`/`libva-drm`/`libva-x11`, `libvulkan`, `libwayland-client`/`-egl`, Mesa, LLVM and
  the NVIDIA libraries (the AppImage excludelist). The host's drivers are built
  against the host's copies of these and look for their backends in the
  host's directories; a bundled copy would win over them through the relative
  RPATH (#625). The test fails if any of them is bundled. The pinned Ubuntu
  FFmpeg source is rebuilt with its original configuration and hardware
  backends: VA mapping uses `vaMapBuffer2` when the host provides it and
  otherwise uses `vaMapBuffer`. Assembly requires this audited libavutil;
  the original library cannot start against Ubuntu 22.04's host libva.
  `AppRun` points `LIBVA_DRIVERS_PATH` at the host's `dri` directories.
  It also sets host GBM backend, EGL vendor/external-platform and Vulkan ICD
  search paths, including `/run/opengl-driver` where present. Existing user
  overrides are preserved. The tests execute this same launcher block.
- **glibc, libstdc++ and libgcc_s are bundled but the host's are used when they
  are at least as new** (#621). The host's Mesa loads LLVM, which needs the
  host's glibc and libstdc++ (Arch: glibc 2.44, GLIBCXX_3.4.36); on the
  bundled 2.43 loader video initialisation failed with `eglGetDisplay failed`.
  The copies live in `glibc/`, outside `usr/lib`; every ELF file's RPATH ends
  with `/tmp/.obs-rust-appimage-<hash>/lib`. `AppRun` compares the host's loader
  version (`ld.so --version`) and libstdc++ (`GLIBCXX_3.4.N`) with the bundled
  ones (`glibc/GLIBC_VERSION`, `glibc/GLIBCXX_MINOR`). If the host's are at
  least as new, it installs the host's loader at the fixed interpreter path and
  leaves that `lib` directory empty, so the host's libc is found; otherwise it
  installs the bundled loader and copies `glibc/*.so*` into `lib`. The AppImage
  thus also runs on distributions older than the build host.
  `OBS_APPIMAGE_BUNDLED_LIBC=1` forces the bundled ones. `AppRun` records its
  choice in `/tmp/.obs-rust-appimage-<hash>/mode`.
- **All of glibc, not just what OBS links.** The host's drivers run on the
  bundled loader and libc and link more of glibc (the NVIDIA driver's `libcuda`
  needs `librt`, `libpthread` and `libdl`). The host's copies of those need
  private symbols of the host's glibc, so on a host with a newer glibc
  `libcuda` failed to load and NVENC was missing. `assemble-appdir.sh` bundles
  every library of the package that owns the bundled `libc.so.6`.
- **No `LD_LIBRARY_PATH`.** It would leak into every process OBS starts
  (`/bin/sh`, `xdg-open`, ...), and those host programs crash when they load the
  bundled, newer glibc. The bundled ELF files use relative RPATHs instead, which
  also win over an `LD_LIBRARY_PATH` set by the user's session.
- **Qt and GIO come only from the bundle** (`usr/bin/qt.conf`, `GIO_MODULE_DIR`).
  Plugins of a different Qt version on the host crash OBS. The build fails if
  the Qt SVG plugins (`qt6-svg-plugins` on Ubuntu 26.04) are missing.
- **The bundled loader lives at a fixed path.** Every executable's ELF
  interpreter is `/tmp/.obs-rust-appimage-<hash>/ld-linux-x86-64.so.2`; the
  kernel cannot expand variables in it. `AppRun` copies the bundled loader there
  on start. This keeps `/proc/self/exe` pointing at the real binary, which OBS
  uses to find its data and plugins, and it makes helper processes such as
  `obs-ffmpeg-mux` use the bundled glibc too.

- **Libraries loaded at runtime are bundled as well**, with the paths `AppRun`
  points them at: PipeWire's modules, SPA plugins and `client.conf`, gconv
  modules. A missing one is not noticed until OBS uses it; without
  `client.conf`, `pw_context_new()` returned NULL and every OBS with a PipeWire
  screen capture in its scene crashed on start (obs-rust/obs-studio#426). On
  Ubuntu 26.04 they come from `libpipewire-0.3-modules`, `libspa-0.2-modules`
  and `pipewire-bin`; `assemble-appdir.sh` fails the build without them.

## Tests

`test-appimage.sh` runs in stock Debian 13, Ubuntu 22.04, Fedora and Arch Linux (glibc newer than the build host's) containers
with none of OBS's dependencies installed, and fails unless all of these pass:

1. **Bundle:** `check-appdir.sh`, and every bundled ELF file resolves all of its
   libraries and symbols with the bundled loader (`LD_BIND_NOW`), and the
   glibc libraries that host GPU drivers link (`librt`, `libpthread`, `libdl`,
   ...) resolve to the bundled copies.
2. **First start:** a new user's OBS opens a window that renders, with Qt, the
   core libraries and the Qt SVG plugins loaded from the AppImage.
3. **Runtime probes:** in `AppRun`'s environment, with the bundled loader and
   libraries, the helper connects to PipeWire and to PulseAudio
   (`pipewire-pulse`), converts text through gconv and talks to the session bus.
4. **Saved profile:** OBS starts with a returning user's profile and scene: a
   PipeWire screen capture, a PipeWire camera and a PulseAudio microphone, and
   `--startrecording`. A private session stands in for the desktop (D-Bus,
   PipeWire, WirePlumber, `pipewire-pulse`), the helper serves a fake
   ScreenCast and Camera portal, and test producers play scrolling colour bars
   (screen), a bouncing ball (camera) and a 440 Hz tone (microphone). OBS must
   stay up, every source must start, and `verify-recording.sh` then requires
   both pictures to keep moving and the tone to be present throughout.

The release job publishes only after the tests pass on every distribution.

### Physical GPU validation

The `gpu` scenario requires a real render node; it fails when the node is
absent and refuses a software renderer or software decoder fallback. Run it
in an Arch or Fedora container with the render node and its matching host
driver devices, libraries and configuration exposed. The ordinary hosted
CI jobs without GPU devices do not count as physical validation.

Download the `obs-appimage-gpu-test` artifact from the same AppImage build.
It contains a helper compiled against the packaged FFmpeg version, a
12-frame 128x72 H264 fixture, and its software-decoded YUV420P reference.
Inside the container, alongside the AppImage and usual test helper:

```bash
GPU_TEST="$PWD/obs-appimage-gpu-test" \
GPU_INPUT="$PWD/gpu-h264.mp4" GPU_REFERENCE="$PWD/gpu-reference.yuv" \
GPU_RENDER_NODE=/dev/dri/renderD128 \
build-aux/appimage/test-appimage.sh candidate.AppImage results/gpu obs-appimage-testkit gpu
```

The helper uses AppRun's selected interpreter and C runtime, imports an
exported GBM DMA-BUF with its modifier, and verifies every pixel in twelve
rendered frames. VAAPI must decode twelve hardware surfaces, download them,
and match the reference bytes exactly. Loaded-library maps must show the
host driver stack and packaged libavcodec/libavutil. Configure the vendor's
device selection when needed; for NVIDIA, the CUDA-visible GPU must match
the supplied render node. Each probe has a bounded timeout.

## Limitations

- **One user per machine for a given build.** The loader directory is created by
  the first user who starts the AppImage. `AppRun` refuses to use a directory
  that belongs to another user (other than root) or that anyone else can write
  to, because it is executed as the interpreter of every process OBS starts.
  A second user therefore gets an error until the directory is gone (it is
  removed when `/tmp` is cleaned, for example on reboot).
- **`/tmp` must allow executing files.** On a system where `/tmp` is mounted
  `noexec` the AppImage cannot start.
- **x86_64 only.**
- **Left out of the AppImage:** the browser source (CEF), VLC, WebRTC, AJA,
  Python scripting and the Intel QSV plugin. QSV probes the Intel GPU through
  the host's libvpl runtime at startup, which no CI runner can exercise; build
  it in with the workflow's `qsv` input.

## Troubleshooting

- If OBS crashes, `AppRun` writes `~/.cache/obs-studio-appimage-crash.txt`
  (signal, system, environment, tail of the OBS log). If `gdb` is installed and
  the crash happened right after start, it runs OBS once more under `gdb` and
  adds the backtrace to the report.
- `OBS_APPIMAGE_DEBUG=1 ./OBS-Studio-*-linux-x86_64.AppImage` prints the environment and, with
  `gdb` installed, a backtrace.

## Updating the pinned `appimagetool`

The workflow downloads a tagged `appimagetool` release and verifies it with
`sha256sum -c`. To update, change `APPIMAGETOOL_VERSION` and
`APPIMAGETOOL_SHA256` in the workflow together; the checksum is the SHA-256 of
`https://github.com/AppImage/appimagetool/releases/download/<tag>/appimagetool-x86_64.AppImage`.

## Release assets and updates

Rust AppImages are named `OBS-Studio-Rust-<reported-version>-linux-x86_64.AppImage`. The daily beta and `continuous` publish the same AppImage and `.zsync` names. Update information targets the canonical Rust `.zsync` pattern in `continuous`; collection copies both files without renaming them.

`asset_metadata.py` verifies the actual executable identity and embedded update target, and checks the zsync filename, URL, length and whole-file SHA-1 against the AppImage before the portability tests. Release-candidate runtime tests also require the update target. Standalone builds may omit update information.
