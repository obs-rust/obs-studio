# AppImage

`.github/workflows/appimage.yaml` builds OBS-Studio-Rust as an
[AppImage](https://appimage.org) and starts it in stock Debian, Ubuntu and
Fedora containers to prove that it runs on other distributions and that its
captures work (see Tests). A push to master publishes it as the `continuous`
pre-release, a tag push attaches it to that tag's release.

| File | Purpose |
| --- | --- |
| `assemble-appdir.sh` | Turns a `cmake --install` tree into an AppDir: bundles Qt plugins and every shared library (including all of glibc and its loader), then calls `set-runpaths.sh`. |
| `set-runpaths.sh` | Gives every bundled ELF file a relative `DT_RPATH` pointing at `usr/lib`. |
| `AppRun` | Entry point. Installs the bundled loader, sets up the environment, runs OBS and writes a crash report if it crashes. |
| `check-appdir.sh` | Fails when a path `AppRun` exports into the AppDir is missing or empty, or when PipeWire's `client.conf`, the modules and SPA plugins it loads, gconv or the Qt xcb plugin are not bundled. Run by `assemble-appdir.sh` (fails the build) and again by the test. |
| `test-appimage.sh` | The distribution test, run inside a stock container (see below). |
| `testkit.c` | Test helper built next to the AppImage: runtime probes, a fake ScreenCast/Camera portal, the test tone. The test runs it with the AppImage's own loader and libraries. |
| `verify-recording.sh` | Checks the recording the test made (outside the container, which must not have ffmpeg). |

## How it works

- **Everything is bundled** except the GPU and hardware video drivers (Mesa,
  NVIDIA, VA-API), which have to come from the host. That includes glibc, so the
  AppImage also runs on distributions older than the build host.
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

`test-appimage.sh` runs in stock Debian 13, Ubuntu 22.04 and Fedora containers
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
- `OBS_APPIMAGE_DEBUG=1 ./OBS_Studio-*.AppImage` prints the environment and, with
  `gdb` installed, a backtrace.

## Updating the pinned `appimagetool`

The workflow downloads a tagged `appimagetool` release and verifies it with
`sha256sum -c`. To update, change `APPIMAGETOOL_VERSION` and
`APPIMAGETOOL_SHA256` in the workflow together; the checksum is the SHA-256 of
`https://github.com/AppImage/appimagetool/releases/download/<tag>/appimagetool-x86_64.AppImage`.
