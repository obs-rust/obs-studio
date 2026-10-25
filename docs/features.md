# Features ledger

Everything OBS-Studio-Rust does differently from upstream OBS Studio, for a
user: new features, changed behavior, and fixed upstream bugs. A pull
request announces a change once; this file keeps the list (#1250).

It covers every pull request merged up to 2026-10-17 (#1250): 698 merged
pull requests were sorted, and CI, test, build, port and refactoring work
was left out.

## How to add an entry

A pull request with a user-visible change adds its line here, in the same
pull request ([coordination.md](rust-port/coordination.md), section 4).
User-visible means: new or changed behavior, a setting, UI, log output, a
protocol field, a supported platform or package, or a fix for a bug a user
can notice. CI, tests, ports with no change in behavior and refactors do
not get a line.

- Put the line in the area that fits, or add an area.
- One line per feature, not per pull request. A follow-up pull request adds
  its link to the existing line, and updates the text if the behavior
  changed.
- Write what changes for the user, not how the code works.
- Format:

  ```markdown
  - **Short name.** What changes for the user. *Platforms*, merge date.
    Issue [#N](https://github.com/obs-rust/obs-studio/issues/N), PRs
    [#N](https://github.com/obs-rust/obs-studio/pull/N).
  ```

  *Platforms* is `All`, or a list of `Linux`, `Windows` and `macOS`. The
  merge date is the date of the first pull request that shipped the
  feature (`YYYY-MM-DD`, UTC).
- A fixed upstream bug goes in [Fixed upstream bugs](#fixed-upstream-bugs),
  in its area and in merge order. It has no bold name: the line starts with
  the symptom that is gone, and ends with the upstream issue
  (obsproject/obs-studio#N, obsproject/obs-websocket#N) when there is one.
- A fix for a bug that the fork itself added goes on the line of the
  feature it fixes, not on a line of its own.

## Features

- **Hybrid Rust/C application build.** Completed Rust ports replace their C
  equivalents in the normal application; unported components remain available.
  Property parameter destructors observe the original parameter during replacement
  (C API compatibility; [#1938](https://github.com/obs-rust/obs-studio/issues/1938),
  [#1939](https://github.com/obs-rust/obs-studio/pull/1939)).
  Property destructors that remove or add groups run each callback once, and a
  removed group stays readable in its own destructor (C API compatibility;
  [#1943](https://github.com/obs-rust/obs-studio/issues/1943)).
  Settings retain valid JSON members when a plugin produces NaN or infinity,
  and recover valid backups when local JSON contains invalid UTF-8.
  *All*, 2026-10-21. Issue
  [#1587](https://github.com/obs-rust/obs-studio/issues/1587), PR
  [#1588](https://github.com/obs-rust/obs-studio/pull/1588). Plugin string
  edits preserve converted strings even when their recorded capacity is zero.
  Issue [#1941](https://github.com/obs-rust/obs-studio/issues/1941), PR
  [#1945](https://github.com/obs-rust/obs-studio/pull/1945). Persistence fix:
  issue [#1940](https://github.com/obs-rust/obs-studio/issues/1940), PR
  [#1946](https://github.com/obs-rust/obs-studio/pull/1946).

  Opening an unreadable config preserves its contents; failed reads keep the
  C API's empty-config result. Issue
  [#1942](https://github.com/obs-rust/obs-studio/issues/1942), PR
  [#1944](https://github.com/obs-rust/obs-studio/pull/1944).

- Rust libobs process pipes fill reads across kernel pipe chunks, matching the C API on Linux and macOS.

- Failed libobs startup reports the initializer that failed; Linux X11
  hotkey initialization reports a failed display connection and its display
  name. *All* (X11 detail on Linux), 2026-10-17. Issue
  [#1355](https://github.com/obs-rust/obs-studio/issues/1355).

### Headless and automation

- **`--disable-shutdown-check` works again.** After an unclean stop (power
  loss, OOM killer, a hard stop of the VM or container), OBS starts without
  the "Crash Detected" dialog and logs the unclean shutdown as a warning on
  line 1. *All*, 2026-10-15. Issue
  [#809](https://github.com/obs-rust/obs-studio/issues/809), PR
  [#810](https://github.com/obs-rust/obs-studio/pull/810).
- **Settings from before OBS 31 are migrated.** Settings without
  `LastVersion` no longer open the first-run wizard, which on a headless
  display was hidden and disabled all outputs. *All*, 2026-10-15. Issue
  [#815](https://github.com/obs-rust/obs-studio/issues/815), PR
  [#816](https://github.com/obs-rust/obs-studio/pull/816).
- **Refused output starts are logged.** A start of streaming, recording, the
  replay buffer or the virtual camera that is refused while outputs are
  disabled writes a log line, where before obs-websocket replied "success"
  and nothing happened. *All*, 2026-10-15. Issue
  [#817](https://github.com/obs-rust/obs-studio/issues/817), PR
  [#818](https://github.com/obs-rust/obs-studio/pull/818).
- **obs-websocket changes are saved.** Requests that change the scene
  collection (volume, mute, input settings, transforms and others) save it,
  so a crash or a hard stop no longer loses them. *All*, 2026-10-16. Issue
  [#811](https://github.com/obs-rust/obs-studio/issues/811), PR
  [#812](https://github.com/obs-rust/obs-studio/pull/812).
- **`obs-stream-rust` command-line tool.** Starts, stops and toggles the
  stream of a running OBS and shows its status through obs-websocket v5,
  with password support, IPv6 hosts and a `--json` flag for scripts. *All*,
  2026-10-10. Issue [#96](https://github.com/obs-rust/obs-studio/issues/96),
  PRs [#30](https://github.com/obs-rust/obs-studio/pull/30),
  [#276](https://github.com/obs-rust/obs-studio/pull/276),
  [#277](https://github.com/obs-rust/obs-studio/pull/277).

### GPU status

- **GPU acceleration light.** A light in the status bar shows whether OBS
  renders on a hardware GPU (green), in software on a machine without a GPU
  (yellow), or in software although a GPU is present, after a graphics
  failure or over the frame budget (red). Clicking it explains the state and
  quotes the graphics log lines, including the D3D11 adapter selected on Windows. Legacy Mesa Software Rasterizer and Apple
  Software Renderer are recognized as CPU rendering. The startup log and
  `GetStats.gpuAcceleration` in obs-websocket have the same state
  ([gpu-status.md](gpu-status.md)). *All*, 2026-10-15. Issues
  [#846](https://github.com/obs-rust/obs-studio/issues/846),
  [#942](https://github.com/obs-rust/obs-studio/issues/942),
  [#943](https://github.com/obs-rust/obs-studio/issues/943),
  [#944](https://github.com/obs-rust/obs-studio/issues/944),
  [#946](https://github.com/obs-rust/obs-studio/issues/946),
  [#1003](https://github.com/obs-rust/obs-studio/issues/1003),
  [#1005](https://github.com/obs-rust/obs-studio/issues/1005),
  [#1225](https://github.com/obs-rust/obs-studio/issues/1225), PRs
  [#890](https://github.com/obs-rust/obs-studio/pull/890),
  [#894](https://github.com/obs-rust/obs-studio/pull/894),
  [#929](https://github.com/obs-rust/obs-studio/pull/929),
  [#958](https://github.com/obs-rust/obs-studio/pull/958),
  [#960](https://github.com/obs-rust/obs-studio/pull/960),
  [#963](https://github.com/obs-rust/obs-studio/pull/963),
  [#1006](https://github.com/obs-rust/obs-studio/pull/1006),
  [#1010](https://github.com/obs-rust/obs-studio/pull/1010),
  [#1018](https://github.com/obs-rust/obs-studio/pull/1018),
  [#1035](https://github.com/obs-rust/obs-studio/pull/1035),
  [#1178](https://github.com/obs-rust/obs-studio/pull/1178),
  [#1226](https://github.com/obs-rust/obs-studio/pull/1226).
- **`GPU: x%` label.** The light shows the render GPU's utilization next to
  the CPU usage, or `CPU rendering` in software; obs-websocket gets the same
  reading. NVML process sampling keeps a separate window for each GPU and PID,
  and falls back to the older API when a GPU cannot use the newer one.
  Sources: NVML for NVIDIA and `gpu_busy_percent` for AMD on
  Linux, IOKit on macOS, the PDH `GPU Engine` counters on Windows (every
  vendor). *All*, 2026-10-17. Issues
  [#1020](https://github.com/obs-rust/obs-studio/issues/1020),
  [#1021](https://github.com/obs-rust/obs-studio/issues/1021),
  [#1022](https://github.com/obs-rust/obs-studio/issues/1022),
  [#1024](https://github.com/obs-rust/obs-studio/issues/1024),
  [#1025](https://github.com/obs-rust/obs-studio/issues/1025),
  [#1026](https://github.com/obs-rust/obs-studio/issues/1026), PRs
  [#1194](https://github.com/obs-rust/obs-studio/pull/1194),
  [#1195](https://github.com/obs-rust/obs-studio/pull/1195),
  [#1198](https://github.com/obs-rust/obs-studio/pull/1198),
  [#1222](https://github.com/obs-rust/obs-studio/pull/1222),
  [#1224](https://github.com/obs-rust/obs-studio/pull/1224),
  [#1228](https://github.com/obs-rust/obs-studio/pull/1228),
  [#1233](https://github.com/obs-rust/obs-studio/pull/1233),
  [#1236](https://github.com/obs-rust/obs-studio/pull/1236),
  [#1238](https://github.com/obs-rust/obs-studio/pull/1238),
  [#1262](https://github.com/obs-rust/obs-studio/pull/1262),
  [#1264](https://github.com/obs-rust/obs-studio/pull/1264).

### Video capture

- **reject duplicate inputs after scaler reentry.** One disconnect removes a video input even when scaler creation reenters registration. *All*, 2026-10-20. Issue [#1529](https://github.com/obs-rust/obs-studio/issues/1529), PR [#1534](https://github.com/obs-rust/obs-studio/pull/1534).
- Virtual camera output resumes at the correct byte after a short device write. *Linux*.

- **Busy V4L2 cameras are recovered.** When another process holds a V4L2
  camera, OBS names that process, offers a Force Camera button and opens the
  camera when it is free. *Linux*, 2026-10-15. Issue
  [#799](https://github.com/obs-rust/obs-studio/issues/799), PR
  [#800](https://github.com/obs-rust/obs-studio/pull/800).
- **Option to not restore V4L2 camera controls.** A V4L2 camera source has
  the option "Restore camera controls when the camera connects", so an
  external PTZ controller can keep control of the camera. *Linux*,
  2026-10-12. Issues
  [#385](https://github.com/obs-rust/obs-studio/issues/385),
  [#976](https://github.com/obs-rust/obs-studio/issues/976), PRs
  [#386](https://github.com/obs-rust/obs-studio/pull/386),
  [#977](https://github.com/obs-rust/obs-studio/pull/977).

- **Video scaler buffer ownership.** A video input replaced during scaling cannot inherit the old registration's scaler buffers. *All*, 2026-10-20. Issue [#1540](https://github.com/obs-rust/obs-studio/issues/1540), PR [#1544](https://github.com/obs-rust/obs-studio/pull/1544).

### Audio

- JACK timestamp fallback uses the active server sample rate instead of an uninitialized zero rate. *Linux*.

- Unchanged JACK source settings no longer restart audio capture. *Linux*.
- Stopping ALSA capture before its worker starts no longer revives the worker and hangs shutdown. *Linux*.

- **ALSA captures 24-bit devices at 24 bits.** A USB audio interface that
  offers `S24_3LE` and `S16_LE` is opened at 24 bits, where before OBS tried
  16 bits first. *Linux*, 2026-10-16. Issue
  [#1103](https://github.com/obs-rust/obs-studio/issues/1103), PR
  [#1104](https://github.com/obs-rust/obs-studio/pull/1104).

### Encoding, streaming and recording

- **Missing encoder reasons.** When a profile names a VA-API or Intel QSV
  encoder this OBS does not have, the Missing Encoders dialog says why
  instead of "Unknown", and how to fix it. For VA-API it names the cause:
  no GPU render device, an NVIDIA GPU (use NVENC), no access to the render
  device (with the `usermod` command), or no VA-API driver installed (with
  the install command for Debian/Ubuntu, Fedora or Arch), and logs it as a
  `VAAPI:` line. A build without QSV (the AppImage) says to pick another
  encoder for Streaming and Recording. *All*, pending merge. Issues
  [#1720](https://github.com/obs-rust/obs-studio/issues/1720),
  [#1862](https://github.com/obs-rust/obs-studio/issues/1862), PRs
  [#1841](https://github.com/obs-rust/obs-studio/pull/1841),
  [#1872](https://github.com/obs-rust/obs-studio/pull/1872).
- **QSV probe log.** On Linux, the OBS log has one `QSV probe:` line per GPU
  render device, with whether VA-API opened it, its driver and whether it
  encodes HEVC and AV1. That is what decides which Quick Sync encoders
  appear, so a log from real Intel hardware is enough to check and pin that
  decision in a test. *Linux*, pending merge. Issue
  [#1863](https://github.com/obs-rust/obs-studio/issues/1863), PR
  [#1879](https://github.com/obs-rust/obs-studio/pull/1879).
- **Missing source types.** When a loaded scene collection has sources whose
  plugin this OBS does not have, a "Missing Source Types" warning lists them
  instead of leaving them blank without a word: browser sources (not in this
  build yet, with the Window Capture workaround), VLC, or the plugin type,
  with how many sources each. The sources keep their settings. Safe mode only
  logs it. *All*, pending merge. Issue
  [#1865](https://github.com/obs-rust/obs-studio/issues/1865), PR
  [#1883](https://github.com/obs-rust/obs-studio/pull/1883).

- **let buffered queues fit one chunk.** Large buffered recording chunks complete when the configured queue limit is smaller than a chunk. *All*, 2026-10-20. Issue [#1526](https://github.com/obs-rust/obs-studio/issues/1526), PR [#1527](https://github.com/obs-rust/obs-studio/pull/1527).

- **FFmpeg output retry and encoder errors.** FFmpeg output can retry after a connection attempt that finishes before thread creation returns, and missing FFmpeg video encoders report an error using initialized message storage. *All*, pending merge. Split from PR [#1464](https://github.com/obs-rust/obs-studio/pull/1464).
- **FFmpeg muxer error handling.** The FFmpeg recording muxer reports seek positions and rejects unsupported seeks correctly, cleans up safely when the recording file cannot be opened, exits with an error when writing or reading a packet fails, and restores argument ownership when opening a split recording fails (prevents a Windows double free). *All*, pending merge. Split from PR [#1464](https://github.com/obs-rust/obs-studio/pull/1464).

- **Codec availability follows the build.** MP4 and MOV outputs do not advertise
  HEVC when it was disabled at build time. *Linux*, pending merge. PR
  [#1333](https://github.com/obs-rust/obs-studio/pull/1333).

- **wake buffered producers after disk errors.** Buffered recording producers wake and report failure after a disk write error. *All*, 2026-10-20. Issue [#1533](https://github.com/obs-rust/obs-studio/issues/1533), PR [#1536](https://github.com/obs-rust/obs-studio/pull/1536).

- **Enhanced Broadcasting encoder controls.** Enhanced Broadcasting stream
  settings can turn off B-frames and lookahead and set the preset and
  tuning, for lower latency than the service settings give, and show a
  read-only preview of the encoding ladder the service sends. *All*,
  2026-10-11. Issue [#19](https://github.com/obs-rust/obs-studio/issues/19),
  PRs [#162](https://github.com/obs-rust/obs-studio/pull/162),
  [#181](https://github.com/obs-rust/obs-studio/pull/181).

### Sources and filters

- **Bundled default font for text.** The Text (FreeType 2) source uses the
  bundled JetBrainsMono Nerd Font as its default on every OS. *All*,
  2026-10-16. Issue
  [#1012](https://github.com/obs-rust/obs-studio/issues/1012), PR
  [#1016](https://github.com/obs-rust/obs-studio/pull/1016).
- **Chromium 156 with H.264/AAC for browser sources.** Linux x86_64 builds
  use CEF 8078 (Chromium 156), built by obs-rust with proprietary codecs,
  so browser sources and panels play H.264/AAC media. *Linux*, 2026-10-23.
  Issue [#1864](https://github.com/obs-rust/obs-studio/issues/1864), PR
  [#1948](https://github.com/obs-rust/obs-studio/pull/1948).
- **Chromium sandbox for browser sources on Linux and macOS.** Browser
  sources and panels run in Chromium's sandbox where it can work. On Linux:
  with unprivileged user namespaces, or with a `chrome-sandbox` helper that
  is setuid root. On macOS: each helper app sandboxes itself (Seatbelt) with
  `libcef_sandbox.dylib` from the CEF framework in the app. Where it can't
  (on Linux, OBS runs as root or neither is available; on macOS, the library
  is missing), they run without it as before, and OBS shows the fallback
  warning with the cause and what to change. *Linux*, 2026-10-25; *macOS*,
  pending merge. Windows: [#2379](https://github.com/obs-rust/obs-studio/issues/2379).
  Issue [#1092](https://github.com/obs-rust/obs-studio/issues/1092), PRs
  [#2378](https://github.com/obs-rust/obs-studio/pull/2378),
  [obs-rust/obs-browser#3](https://github.com/obs-rust/obs-browser/pull/3),
  [obs-rust/obs-browser#8](https://github.com/obs-rust/obs-browser/pull/8),
  [obs-rust/obs-browser#12](https://github.com/obs-rust/obs-browser/pull/12).

### Scenes and UI

- **accept C hexadecimal prefixes in config integers.** Configuration integer values accept the optional hexadecimal prefix after the sign, matching C. *All*, 2026-10-20. Issue [#1531](https://github.com/obs-rust/obs-studio/issues/1531), PR [#1532](https://github.com/obs-rust/obs-studio/pull/1532).
- Remux queue path and processing-state changes notify views with valid cell ranges. *All*.

- The remux queue updates its insertion row consistently when processing starts and finishes. *All*.

- **Wayland global hotkeys.** On Wayland, hotkeys and push-to-talk work
  while another window has focus, through the desktop's GlobalShortcuts
  portal. Each shortcut is named by its actions in the desktop's shortcut
  list, Settings > Hotkeys shows whether global hotkeys work (with a marker
  on each key) and can open the desktop's shortcut editor. *Linux*,
  2026-10-11. Issues
  [#240](https://github.com/obs-rust/obs-studio/issues/240),
  [#255](https://github.com/obs-rust/obs-studio/issues/255),
  [#347](https://github.com/obs-rust/obs-studio/issues/347),
  [#408](https://github.com/obs-rust/obs-studio/issues/408),
  [#474](https://github.com/obs-rust/obs-studio/issues/474),
  [#790](https://github.com/obs-rust/obs-studio/issues/790), PRs
  [#245](https://github.com/obs-rust/obs-studio/pull/245),
  [#256](https://github.com/obs-rust/obs-studio/pull/256),
  [#348](https://github.com/obs-rust/obs-studio/pull/348),
  [#409](https://github.com/obs-rust/obs-studio/pull/409),
  [#475](https://github.com/obs-rust/obs-studio/pull/475),
  [#791](https://github.com/obs-rust/obs-studio/pull/791).
- **Help → Discord opens the OBS Rust server.** The Help menu's Discord
  item opens the OBS Rust Discord, where this build's bugs and questions
  belong, instead of upstream OBS Studio's server. *All*, 2026-10-24.
  Issue [#2097](https://github.com/obs-rust/obs-studio/issues/2097).
- **Fallback warning.** When a feature falls back to a less safe mode so
  that it keeps working (for example a browser source without the Chromium
  sandbox), OBS logs it at error level and shows a warning with a 3-second
  countdown that accepts the fallback, once per session. Plugins call
  `obs_frontend_warn_fallback()` ([fallback-warning.md](fallback-warning.md));
  the browser source uses it when the Chromium sandbox can't work. *All*,
  2026-10-25. Issue
  [#2218](https://github.com/obs-rust/obs-studio/issues/2218), PRs
  [#2238](https://github.com/obs-rust/obs-studio/pull/2238),
  [obs-rust/obs-browser#8](https://github.com/obs-rust/obs-browser/pull/8).
- **No upstream release notes or donor list downloads.** The What's New
  dialog, which showed upstream OBS Studio's release notes from
  obsproject.com, is off by default (`ENABLE_WHATSNEW=OFF`); a build that
  turns it on reads `whatsnew.json` from the obs-rust release assets
  ([#1882](https://github.com/obs-rust/obs-studio/issues/1882)). The About
  box no longer downloads upstream's Patreon list; it links to the
  obs-rust repository instead. Two obsproject.com data feeds stay on
  because they are data only and useful: the streaming-service list
  (`plugins/rtmp-services/data/package.json`) and the game-capture
  compatibility list (`plugins/win-capture/data/package.json`).
  `ci/test_no_upstream_runtime_feeds.py` keeps the allow-list. *All*,
  2026-10-21. Issue
  [#1732](https://github.com/obs-rust/obs-studio/issues/1732).

### Performance

- **Volume meters repaint only when they change.** The audio mixer's meters
  no longer repaint 60 times a second while the mixer is hidden, OBS is
  minimized or every source is silent; a meter stops about 20 s after its
  last sound, when its peak hold falls back. *All*, 2026-10-17. Issue
  [#1281](https://github.com/obs-rust/obs-studio/issues/1281), PR
  [#1307](https://github.com/obs-rust/obs-studio/pull/1307).
- **Faster launch on NVIDIA machines.** 4.3 times faster to the main window
  and 19 % less memory on an NVIDIA streaming machine (see the README): the
  VAAPI probe skips decode-only drivers, the NVENC test result is reused
  while the driver does not change, the AppImage loads only NVIDIA's EGL
  vendor, and the GPU status probe runs on a thread. *Linux*, 2026-10-17.
  Issues [#1168](https://github.com/obs-rust/obs-studio/issues/1168),
  [#1170](https://github.com/obs-rust/obs-studio/issues/1170),
  [#1171](https://github.com/obs-rust/obs-studio/issues/1171),
  [#1172](https://github.com/obs-rust/obs-studio/issues/1172), PRs
  [#1175](https://github.com/obs-rust/obs-studio/pull/1175),
  [#1176](https://github.com/obs-rust/obs-studio/pull/1176),
  [#1177](https://github.com/obs-rust/obs-studio/pull/1177),
  [#1178](https://github.com/obs-rust/obs-studio/pull/1178).
- **Settings load and save faster.** Storing and reloading scene collections,
  profiles and source settings no longer takes about a third longer than the
  original C code: the JSON round trip is 28-32 % faster than before. *All*,
  2026-10-19. Issue
  [#1452](https://github.com/obs-rust/obs-studio/issues/1452), PR
  [#1461](https://github.com/obs-rust/obs-studio/pull/1461).
- **Formatted UI text builds faster.** Messages that substitute `$1`..`$4`
  values no longer rescan and rebuild the string once per value: 27 % faster
  for a short message and 5 % faster for a 4 KB one, both now ahead of the
  original C code. *All*, 2026-10-21. Issue
  [#1296](https://github.com/obs-rust/obs-studio/issues/1296), PR
  [#1619](https://github.com/obs-rust/obs-studio/pull/1619).

### Stability

- Repeated null-output encoding sessions reclaim completed stop threads. *All*.
- **No swscale warning for every JPEG.** Loading a JPEG image (Image
  Source, Image Slide Show) no longer logs FFmpeg's "deprecated pixel
  format used, make sure you did set range correctly" once per image or
  slide; the colours are unchanged. *All*, pending merge. Issue
  [#2334](https://github.com/obs-rust/obs-studio/issues/2334).

- **preserve empty item types for defaults.** Empty settings items retain their type when an incompatible default is supplied. *All*, 2026-10-20. Issue [#1528](https://github.com/obs-rust/obs-studio/issues/1528), PR [#1530](https://github.com/obs-rust/obs-studio/pull/1530).
- Callback parameters that exactly fill a fixed calldata buffer are preserved. *All*.
- Signal handlers release their storage when the last reference-holding callback removes itself. *All*.
- Global signal callbacks can disconnect themselves during delivery. *All*.
- Callbacks that emit nested signals can still remove the currently executing callback. *All*.
- Recursive emission of a signal no longer skips remaining subscribers after a callback removes itself. *All*.
- Signal callbacks can register additional callbacks and then remove themselves safely. *All*.
- Global signal callbacks can register additional global callbacks and then remove themselves safely. *All*.
- Global signal callbacks that emit nested signals or disconnect other global callbacks no longer skip or remove the next global callback. *All*.

- **Diagnostics when video fails to start.** When video fails to start on
  Wayland, the log shows the EGL error and the display variables, and OBS
  cleans up without leaks. *Linux*, 2026-10-13. Issue
  [#633](https://github.com/obs-rust/obs-studio/issues/633), PR
  [#655](https://github.com/obs-rust/obs-studio/pull/655).
- **Slow GPU usage sources do not stall the label.** After a slow reading
  the `GPU: x%` label is updated less often (up to every 30 s), a reading
  that is too old shows `GPU: n/a` with "The GPU reading is late.", and a
  source that is slow three times in a row is not asked again in that
  session, with one log line. *All*, 2026-10-17. Issue
  [#1022](https://github.com/obs-rust/obs-studio/issues/1022), PR
  [#1287](https://github.com/obs-rust/obs-studio/pull/1287).
- **Recordings no longer hold the stream's connection or the QSV device.**
  A recording started while streaming no longer keeps a copy of the RTMP
  connection in its muxer, nor of the QSV encoder's GPU device on Linux,
  and a VA-API or QSV device that has no driver is closed again.
  *Linux, macOS*, pending merge. Issue
  [#2305](https://github.com/obs-rust/obs-studio/issues/2305).
- **Profiles with the same name are all listed.** Importing a profile
  named like an existing one gives it a name of its own (for example
  "Name2"), and two profile folders with the same name both show up in
  the Profile menu, the second under a unique name with a warning in the
  log; before, one of them was hidden and could not be selected, renamed
  or removed. *All*, pending merge. Issue
  [#2382](https://github.com/obs-rust/obs-studio/issues/2382).
- **QSV encoders release the GPU when they stop.** Each QSV encoder
  start on Linux, including a failed one, no longer leaks the Intel VPL
  loader and session; a start that fails no longer keeps the VA display
  and the GPU device open. *Linux*, pending merge. Issue
  [#2393](https://github.com/obs-rust/obs-studio/issues/2393).
- **Failed QSV starts release what they took.** A QSV encoder start
  whose Intel VPL session can't be created no longer leaks the VPL
  loader, and on Linux a failed frame allocation no longer keeps its VA
  surfaces and textures. *All*, pending merge. Issue
  [#2396](https://github.com/obs-rust/obs-studio/issues/2396).

### Platforms and packages

- **Logs and crash reports stay on your computer.** OBS-Studio-Rust no
  longer sends logs or crash reports to obsproject.com and has no log
  analyzer button. Help > Log Files > Upload and the crash prompt open the
  file's folder and ask you to attach it to a new issue at
  github.com/obs-rust/obs-studio/issues. *All*, 2026-10-21. Issue
  [#1731](https://github.com/obs-rust/obs-studio/issues/1731).
- **report buffered close errors when copying files.** File copies report buffered close failures instead of success for truncated output. *Linux, macOS*, 2026-10-20. Issue [#1537](https://github.com/obs-rust/obs-studio/issues/1537), PR [#1538](https://github.com/obs-rust/obs-studio/pull/1538).

- **Own app identity.** The fork runs as OBS-Studio-Rust with its own logo,
  settings folder (`obs-studio-rust`) and version, next to the upstream OBS
  version it is based on, in the window title, log, About dialog,
  `--version` and package metadata. On first run it copies the upstream OBS
  settings without changing them. Plugins still see a valid OBS version.
  On macOS the browser source's helper apps take the app's name too, so
  adding a browser source no longer crashes OBS. *All*, 2026-10-10. Issues
  [#111](https://github.com/obs-rust/obs-studio/issues/111),
  [#631](https://github.com/obs-rust/obs-studio/issues/631),
  [#632](https://github.com/obs-rust/obs-studio/issues/632),
  [#1087](https://github.com/obs-rust/obs-studio/issues/1087),
  [#2386](https://github.com/obs-rust/obs-studio/issues/2386), PRs
  [#67](https://github.com/obs-rust/obs-studio/pull/67),
  [#112](https://github.com/obs-rust/obs-studio/pull/112),
  [#510](https://github.com/obs-rust/obs-studio/pull/510),
  [#515](https://github.com/obs-rust/obs-studio/pull/515),
  [#657](https://github.com/obs-rust/obs-studio/pull/657),
  [#706](https://github.com/obs-rust/obs-studio/pull/706),
  [#1088](https://github.com/obs-rust/obs-studio/pull/1088),
  [#2390](https://github.com/obs-rust/obs-studio/pull/2390),
  [obs-rust/obs-browser#9](https://github.com/obs-rust/obs-browser/pull/9).
- **Daily beta packages.** A beta is published every day as the latest
  release: an AppImage for Linux, zips for Windows x64 (with the Visual C++
  runtime) and ARM64, and DMGs for Apple silicon and Intel Macs. *All*,
  2026-10-13. Issues
  [#626](https://github.com/obs-rust/obs-studio/issues/626),
  [#627](https://github.com/obs-rust/obs-studio/issues/627),
  [#628](https://github.com/obs-rust/obs-studio/issues/628), PRs
  [#639](https://github.com/obs-rust/obs-studio/pull/639),
  [#642](https://github.com/obs-rust/obs-studio/pull/642),
  [#664](https://github.com/obs-rust/obs-studio/pull/664),
  [#679](https://github.com/obs-rust/obs-studio/pull/679).
- **AppImage for most Linux distributions.** The AppImage is built on glibc
  2.28 and uses the host's GPU and VA-API drivers, so it starts and encodes
  in hardware on older and current distributions. It bundles PipeWire, finds
  the host's CA certificates (HTTPS works on Debian, Ubuntu, Arch and
  Fedora), and does not load the plugins of a distribution's OBS on the same
  machine. *Linux*, 2026-10-13. Issues
  [#426](https://github.com/obs-rust/obs-studio/issues/426),
  [#498](https://github.com/obs-rust/obs-studio/issues/498),
  [#621](https://github.com/obs-rust/obs-studio/issues/621),
  [#625](https://github.com/obs-rust/obs-studio/issues/625),
  [#926](https://github.com/obs-rust/obs-studio/issues/926),
  [#1084](https://github.com/obs-rust/obs-studio/issues/1084),
  [#1085](https://github.com/obs-rust/obs-studio/issues/1085), PRs
  [#437](https://github.com/obs-rust/obs-studio/pull/437),
  [#499](https://github.com/obs-rust/obs-studio/pull/499),
  [#650](https://github.com/obs-rust/obs-studio/pull/650),
  [#722](https://github.com/obs-rust/obs-studio/pull/722),
  [#927](https://github.com/obs-rust/obs-studio/pull/927),
  [#1227](https://github.com/obs-rust/obs-studio/pull/1227),
  [#1234](https://github.com/obs-rust/obs-studio/pull/1234).
- **.deb version is the release version.** A release .deb's package
  version is the OBS-Studio-Rust version (`1:0.1.0~beta.20261024`), not
  upstream's `git describe` string, so `dpkg -l` matches the About box and
  each daily installs as an upgrade. *Linux*, 2026-10-24. Issue
  [#2299](https://github.com/obs-rust/obs-studio/issues/2299).
- **WHIP streaming in the AppImage.** The AppImage ships obs-webrtc and its
  libdatachannel, so WHIP appears in Settings > Stream > Service. *Linux*,
  2026-10-23. Issue [#1744](https://github.com/obs-rust/obs-studio/issues/1744),
  PR [#1871](https://github.com/obs-rust/obs-studio/pull/1871).
- **AppImage names a missing libwayland-egl.** On a host without
  libwayland-egl.so.1, the AppImage says which package to install
  (libwayland-egl1 on Debian and Ubuntu) instead of "Failed to initialize
  video: Graphics module not found". *Linux (AppImage)*, pending merge.
  Issue [#2298](https://github.com/obs-rust/obs-studio/issues/2298).
- **AppImage names a missing libGLX.** On a host without libGLX.so.0, the
  AppImage says which package to install (libglx0 on Debian and Ubuntu)
  instead of the dynamic loader's "error while loading shared libraries".
  *Linux (AppImage)*, pending merge. Issue
  [#2337](https://github.com/obs-rust/obs-studio/issues/2337).
- **Builds with Qt 6.8.** The frontend builds against distribution Qt 6.8,
  for example on Debian 13. *Linux*, 2026-10-16. Issue
  [#1015](https://github.com/obs-rust/obs-studio/issues/1015), PR
  [#1017](https://github.com/obs-rust/obs-studio/pull/1017).
- **Rust libobs.** Parts of libobs are ported to Rust with the same
  exported symbols as the C libobs. It began as an opt-in
  `libobs-with-rust` build; since #1588 the hybrid Rust/C `libobs` is the
  application default (see "Hybrid Rust/C application build"), and the
  C-only `libobs-c-reference` is built only on request for tests. *All*,
  2026-10-13. Issues
  [#4](https://github.com/obs-rust/obs-studio/issues/4),
  [#453](https://github.com/obs-rust/obs-studio/issues/453),
  [#1065](https://github.com/obs-rust/obs-studio/issues/1065), PRs
  [#454](https://github.com/obs-rust/obs-studio/pull/454),
  [#1086](https://github.com/obs-rust/obs-studio/pull/1086).

- **No updates from upstream OBS Studio on macOS.** The macOS app no
  longer checks upstream's update feed, so it can't offer to replace itself
  with upstream OBS Studio. Its in-app updater stays off until OBS-Studio-Rust
  has its own feed. *macOS*, 2026-10-22. PR
  [#1727](https://github.com/obs-rust/obs-studio/pull/1727).

- **In-app updates from certified OBS-Studio-Rust releases.** The Windows
  and macOS updaters are on and offer only certified releases from
  obs-rust/obs-studio, never a daily beta or upstream OBS Studio. Daily betas
  are GitHub pre-releases; a maintainer certifies one, which attaches an
  RSA-signed update manifest and EdDSA-signed Sparkle appcasts and makes it
  the Latest release. Only a daily whose release run passed every job and
  that has been out at least two days can be certified (#1757). Windows downloads the release ZIP, checks its SHA-256
  against the signed manifest, installs it and restarts. *Windows, macOS*,
  2026-10-22. PR [#1742](https://github.com/obs-rust/obs-studio/pull/1742).

- **AppImage updates follow certified releases.** The AppImage's embedded
  update information (AppImageUpdate, `.zsync` deltas) points at the
  certified Latest release, like the Windows and macOS updaters, instead of
  the daily `continuous` build. *Linux (AppImage)*, 2026-10-23. Issue
  [#1881](https://github.com/obs-rust/obs-studio/issues/1881).

- **Opt-in nightly updates.** Settings > General > Updates > Update Channel
  offers **Nightly** next to **Stable** (the default). On Nightly, the
  Windows and macOS updaters offer each day's build as soon as it passes the
  release gate, from a signed feed on the rolling `continuous` pre-release,
  instead of waiting for a certified release. Switching back to Stable never
  downgrades. *Windows, macOS*, 2026-10-22. Issue
  [#1759](https://github.com/obs-rust/obs-studio/issues/1759).

- **Hybrid daily builds ship as nightlies before release approval.** The
  daily OBS-Studio-Rust pre-release, the `continuous` AppImage and the
  nightly update feed publish while the hybrid release approval is pending.
  Certifying a release (Latest, the stable feed) still needs the maintainer's
  approved `hybrid-release-evidence.json` and dispatch. *All platforms*,
  2026-10-22. Issue [#1811](https://github.com/obs-rust/obs-studio/issues/1811).

- **Releases ship on dispatch with automated gates only.** Nightlies always
  publish the hybrid build; a public release is published when the maintainer
  dispatches certify and the automated CI gates pass. The manual
  `hybrid-release-evidence.json` approval is gone; regressions are fixed
  forward and covered by new automated tests. *All platforms*, 2026-10-22.
  Issue [#1818](https://github.com/obs-rust/obs-studio/issues/1818).

- **Main window title names OBS Studio Rust.** The title bar reads "OBS Studio
  Rust <version>": the release version, or for development builds the newest
  `obs-studio-rust-v*` tag plus `-dev+<commits>-g<sha>` (or `dev` without a
  tag). Profile, scene collection, safe mode and portable mode still follow;
  `--version` and Help > About keep the upstream base version.
  *All platforms*, 2026-10-23. Issue [#1983](https://github.com/obs-rust/obs-studio/issues/1983).
- **A hung recording share can't stall obs-websocket.** Free disk space
  answers within 200 ms: one query per path runs at a time, and while a
  network share hangs, callers get its last value (0 before the first). So
  clients polling `GetStats` no longer fill obs-websocket's worker pool and
  stop every request and event. *All platforms*, 2026-10-25.
  Issue [#1345](https://github.com/obs-rust/obs-studio/issues/1345).
- **The .deb brings its desktop helpers.** It recommends xdg-utils and
  xdg-desktop-portal with a backend, so on a minimal Ubuntu install Show
  Recordings, log and dialog links, and Wayland capture work after
  `apt install`; it suggests v4l2loopback-dkms for the virtual camera.
  *Linux*, 2026-10-24. Issue [#2301](https://github.com/obs-rust/obs-studio/issues/2301).
- **Software centres link to obs-rust.** The Linux AppStream metainfo names
  obs-rust as developer and points its homepage, bug tracker and source links
  at obs-rust/obs-studio, and the .deb's Homepage and Maintainer follow, so
  "Report an issue" in GNOME Software or KDE Discover reaches this project.
  *Linux*, 2026-10-24. Issue [#2302](https://github.com/obs-rust/obs-studio/issues/2302).
- **The .deb installs next to the distribution's OBS.** On Linux the
  helpers are `obs-studio-rust-ffmpeg-mux` and `obs-studio-rust-nvenc-test`,
  so `apt install` no longer fails over `/usr/bin/obs-nvenc-test` when
  Ubuntu's obs-studio is installed, in either order.
  *Linux*, 2026-10-24. Issue [#2296](https://github.com/obs-rust/obs-studio/issues/2296).

## Fixed upstream bugs

Bugs inherited from OBS Studio (and obs-websocket) that are fixed here, by
area.

### obs-websocket

- Deleting a scene in the Scenes dock sends SceneListChanged once, not
  twice. *All*, 2026-10-13. Issue
  [#534](https://github.com/obs-rust/obs-studio/issues/534), PR
  [#535](https://github.com/obs-rust/obs-studio/pull/535), upstream
  [obsproject/obs-websocket#979](https://github.com/obsproject/obs-websocket/issues/979).
- Setting the Studio Mode T-Bar through the frontend API (obs-websocket
  SetTBarPosition) moves the T-Bar, and releasing it no longer cancels the
  transition. *All*, 2026-10-13. Issue
  [#549](https://github.com/obs-rust/obs-studio/issues/549), PR
  [#550](https://github.com/obs-rust/obs-studio/pull/550), upstream
  [obsproject/obs-studio#11372](https://github.com/obsproject/obs-studio/issues/11372).
- Removing the current profile now sends ProfileListChanged, and
  obs-websocket RemoveProfile removes the current profile without leaving a
  confirmation dialog open on the OBS machine. *All*, 2026-10-13. Issue
  [#568](https://github.com/obs-rust/obs-studio/issues/568), PR
  [#569](https://github.com/obs-rust/obs-studio/pull/569), upstream
  [obsproject/obs-websocket#1267](https://github.com/obsproject/obs-websocket/issues/1267).
- obs-websocket SetSceneItemTransform accepts the transform that
  GetSceneItemTransform returns, with 0 bounds and OBS_BOUNDS_NONE. *All*,
  2026-10-13. Issue
  [#540](https://github.com/obs-rust/obs-studio/issues/540), PR
  [#589](https://github.com/obs-rust/obs-studio/pull/589), upstream
  [obsproject/obs-websocket#1299](https://github.com/obsproject/obs-websocket/issues/1299),
  [obsproject/obs-studio#13124](https://github.com/obsproject/obs-studio/issues/13124).
- obs-websocket GetSourceScreenshot labels jpg and other screenshots with
  the correct MIME type, for example image/jpeg. *All*, 2026-10-13. Issue
  [#523](https://github.com/obs-rust/obs-studio/issues/523), PR
  [#652](https://github.com/obs-rust/obs-studio/pull/652), upstream
  [obsproject/obs-websocket#1298](https://github.com/obsproject/obs-websocket/issues/1298).
- obs-websocket CreateProfile and RemoveProfile answer after the profile
  list has changed, so the next request sees the new profiles. *All*,
  2026-10-15. Issue
  [#804](https://github.com/obs-rust/obs-studio/issues/804), PR
  [#805](https://github.com/obs-rust/obs-studio/pull/805), upstream
  [obsproject/obs-studio#1283](https://github.com/obsproject/obs-studio/issues/1283).
- A Parallel request batch returns each result with its own requestType and
  requestId, so clients get the correct data. *All*, 2026-10-16. Issue
  [#813](https://github.com/obs-rust/obs-studio/issues/813), PR
  [#1074](https://github.com/obs-rust/obs-studio/pull/1074), upstream
  [obsproject/obs-studio#1317](https://github.com/obsproject/obs-studio/issues/1317).
- The Session Duration column in WebSocket Server Settings keeps counting
  after the first minute instead of going blank. *All*, 2026-10-17. Issue
  [#1116](https://github.com/obs-rust/obs-studio/issues/1116), PR
  [#1145](https://github.com/obs-rust/obs-studio/pull/1145), upstream
  [obsproject/obs-studio#1208](https://github.com/obsproject/obs-studio/issues/1208).
- GetSourceScreenshot and SaveSourceScreenshot of a 10-bit or 12-bit source
  return the colors that OBS shows, not a much darker picture. *All*,
  2026-10-17. Issue
  [#1141](https://github.com/obs-rust/obs-studio/issues/1141), PR
  [#1146](https://github.com/obs-rust/obs-studio/pull/1146), upstream
  [obsproject/obs-websocket#1213](https://github.com/obsproject/obs-websocket/issues/1213),
  [obsproject/obs-websocket#1257](https://github.com/obsproject/obs-websocket/issues/1257),
  [obsproject/obs-websocket#1298](https://github.com/obsproject/obs-websocket/issues/1298).
- GetCurrentProgramScene no longer aborts OBS when the program scene was
  removed in Studio Mode; it returns null names and UUIDs. *All*,
  2026-10-17. Issue
  [#1254](https://github.com/obs-rust/obs-studio/issues/1254), PR
  [#1256](https://github.com/obs-rust/obs-studio/pull/1256), upstream
  [obsproject/obs-studio#1349](https://github.com/obsproject/obs-studio/issues/1349).
- obs-websocket GetInputPropertiesListPropertyItems no longer aborts OBS
  when a list item has no name or no value, such as the placeholder at the
  top of the macOS Screen Capture display and application lists; those items
  are reported with empty strings. *All*, 2026-10-23. Issue
  [#1266](https://github.com/obs-rust/obs-studio/issues/1266), PR
  [#1996](https://github.com/obs-rust/obs-studio/pull/1996), upstream
  [obsproject/obs-studio#13905](https://github.com/obsproject/obs-studio/issues/13905).
- A RequestBatch with the Parallel execution type no longer crashes OBS
  when its last request finishes. *All*, 2026-10-23. PR
  [#1996](https://github.com/obs-rust/obs-studio/pull/1996).
- obs-websocket GetStats reports each client's CPU usage since that client's
  own previous GetStats, not since the last one from any client, and clients
  polling at the same time no longer race on one shared state. *All*,
  2026-10-23. Issue
  [#1346](https://github.com/obs-rust/obs-studio/issues/1346), PR
  [#1996](https://github.com/obs-rust/obs-studio/pull/1996).
- obs-websocket RemoveInput removes the scene items of the input at once,
  also in scenes that are not on program or preview, so GetSceneItemList
  and GetInputList no longer show the removed input. *All*, 2026-10-24.
  Issue [#1613](https://github.com/obs-rust/obs-studio/issues/1613), PR
  [#2196](https://github.com/obs-rust/obs-studio/pull/2196).

### Video capture

- **deliver video after a callback disconnects.** A video callback that disconnects itself no longer causes the next subscriber to miss a frame. *All*, 2026-10-20. Issue [#1535](https://github.com/obs-rust/obs-studio/issues/1535), PR [#1539](https://github.com/obs-rust/obs-studio/pull/1539).

- An XComposite window capture no longer asks the X server whether its
  window exists once or twice per frame while holding the graphics
  context; it keeps the window's state from X events (map, unmap,
  destroy) and checks with the server at most once a second, so a busy or
  remote X server no longer stalls rendering. *Linux*, 2026-10-17. Issue
  [#1356](https://github.com/obs-rust/obs-studio/issues/1356), PR
  [#1377](https://github.com/obs-rust/obs-studio/pull/1377).
- An XComposite window capture whose window is minimized or on another
  workspace no longer searches every window and rebuilds its pixmap every
  2 s; the search backs off to 30 s and runs at once when the window maps
  again or the settings change. *Linux*, 2026-10-17. Issue
  [#1357](https://github.com/obs-rust/obs-studio/issues/1357), PR
  [#1366](https://github.com/obs-rust/obs-studio/pull/1366).
- After a PipeWire restart, PipeWire screen capture and PulseAudio audio
  sources reconnect automatically; the screen shows its last frame until
  capture returns. *Linux*, 2026-10-10. Issue
  [#108](https://github.com/obs-rust/obs-studio/issues/108), PR
  [#109](https://github.com/obs-rust/obs-studio/pull/109).
- A capture device that reports more audio buffers than OBS supports no
  longer corrupts the audio frame; the extra buffers are dropped with a log
  warning. *macOS*, 2026-10-11. Issue
  [#92](https://github.com/obs-rust/obs-studio/issues/92), PR
  [#124](https://github.com/obs-rust/obs-studio/pull/124).
- Removing or closing a macOS screen or audio capture (ScreenCaptureKit)
  source no longer crashes OBS. *macOS*, 2026-10-11. Issue
  [#93](https://github.com/obs-rust/obs-studio/issues/93), PR
  [#125](https://github.com/obs-rust/obs-studio/pull/125).
- A V4L2 Video Capture Device runs at the selected frame rate, and changing
  camera controls such as zoom or focus no longer restarts the capture.
  *Linux*, 2026-10-11. Issue
  [#201](https://github.com/obs-rust/obs-studio/issues/201), PR
  [#202](https://github.com/obs-rust/obs-studio/pull/202), upstream
  [obsproject/obs-studio#13388](https://github.com/obsproject/obs-studio/issues/13388),
  [obsproject/obs-studio#13896](https://github.com/obsproject/obs-studio/issues/13896).
- V4L2 devices whose input capabilities cannot be read are closed again, so
  cameras no longer stop opening with "more than 16 video devices". *Linux*,
  2026-10-11. Issue
  [#203](https://github.com/obs-rust/obs-studio/issues/203), PR
  [#204](https://github.com/obs-rust/obs-studio/pull/204), upstream
  [obsproject/obs-studio#5422](https://github.com/obsproject/obs-studio/issues/5422).
- The Linux virtual camera sends a correct picture to a v4l2loopback device
  with a fixed format of a different size. *Linux*, 2026-10-11. Issue
  [#212](https://github.com/obs-rust/obs-studio/issues/212), PR
  [#213](https://github.com/obs-rust/obs-studio/pull/213), upstream
  [obsproject/obs-studio#12081](https://github.com/obsproject/obs-studio/issues/12081).
- A PipeWire screen capture no longer crashes OBS when a portal reply
  arrives while the source is removed. *Linux*, 2026-10-11. Issue
  [#246](https://github.com/obs-rust/obs-studio/issues/246), PR
  [#247](https://github.com/obs-rust/obs-studio/pull/247), upstream
  [obsproject/obs-studio#13809](https://github.com/obsproject/obs-studio/issues/13809).
- PipeWire screen capture no longer draws a black square around the cursor
  with Rec. 2100 (HDR) output. *Linux*, 2026-10-12. Issue
  [#328](https://github.com/obs-rust/obs-studio/issues/328), PR
  [#329](https://github.com/obs-rust/obs-studio/pull/329), upstream
  [obsproject/obs-studio#13350](https://github.com/obsproject/obs-studio/issues/13350).
- A V4L2 camera keeps capturing after a corrupt MJPEG or H.264 frame instead
  of stopping until OBS restarts. *Linux*, 2026-10-12. Issue
  [#330](https://github.com/obs-rust/obs-studio/issues/330), PR
  [#331](https://github.com/obs-rust/obs-studio/pull/331), upstream
  [obsproject/obs-studio#12521](https://github.com/obsproject/obs-studio/issues/12521).
- You can change a V4L2 camera's resolution, frame rate or format while
  recording; the camera no longer stays busy. *Linux*, 2026-10-12. Issue
  [#339](https://github.com/obs-rust/obs-studio/issues/339), PR
  [#340](https://github.com/obs-rust/obs-studio/pull/340), upstream
  [obsproject/obs-studio#12508](https://github.com/obsproject/obs-studio/issues/12508).
- Saved V4L2 camera controls (zoom, focus, exposure) are applied when OBS
  starts, when the camera reconnects and when a script changes them.
  *Linux*, 2026-10-12. Issues
  [#363](https://github.com/obs-rust/obs-studio/issues/363),
  [#459](https://github.com/obs-rust/obs-studio/issues/459), PRs
  [#364](https://github.com/obs-rust/obs-studio/pull/364),
  [#460](https://github.com/obs-rust/obs-studio/pull/460), upstream
  [obsproject/obs-studio#11633](https://github.com/obsproject/obs-studio/issues/11633),
  [obsproject/obs-studio#3003](https://github.com/obsproject/obs-studio/issues/3003).
- A PipeWire camera that sends NV12 shows its picture instead of green bands
  or a crash. *Linux*, 2026-10-12. Issue
  [#390](https://github.com/obs-rust/obs-studio/issues/390), PR
  [#391](https://github.com/obs-rust/obs-studio/pull/391), upstream
  [obsproject/obs-studio#13823](https://github.com/obsproject/obs-studio/issues/13823).
- A Wayland screen capture or window capture starts again by itself after
  the monitor sleeps, is unplugged, or the captured window animates, and
  does not stay frozen on the last frame. *Linux*, 2026-10-12. Issues
  [#257](https://github.com/obs-rust/obs-studio/issues/257),
  [#398](https://github.com/obs-rust/obs-studio/issues/398), PRs
  [#399](https://github.com/obs-rust/obs-studio/pull/399),
  [#461](https://github.com/obs-rust/obs-studio/pull/461), upstream
  [obsproject/obs-studio#12063](https://github.com/obsproject/obs-studio/issues/12063).
- PipeWire cameras that send only I420 or YV12 show a picture, also at odd
  widths, and the preview no longer stays black. *Linux*, 2026-10-12. Issues
  [#406](https://github.com/obs-rust/obs-studio/issues/406),
  [#467](https://github.com/obs-rust/obs-studio/issues/467), PRs
  [#407](https://github.com/obs-rust/obs-studio/pull/407),
  [#468](https://github.com/obs-rust/obs-studio/pull/468), upstream
  [obsproject/obs-studio#13744](https://github.com/obsproject/obs-studio/issues/13744).
- PipeWire screen capture from shared memory follows the buffer stride and
  offset, so padded or offset frames are no longer skewed and a short buffer
  no longer crashes OBS. *Linux*, 2026-10-13. Issue
  [#508](https://github.com/obs-rust/obs-studio/issues/508), PR
  [#509](https://github.com/obs-rust/obs-studio/pull/509), upstream
  [obsproject/obs-studio#13318](https://github.com/obsproject/obs-studio/issues/13318).
- PipeWire screen capture shows each frame once, without repeated and
  skipped frames. *Linux*, 2026-10-13. Issue
  [#191](https://github.com/obs-rust/obs-studio/issues/191), PR
  [#612](https://github.com/obs-rust/obs-studio/pull/612), upstream
  [obsproject/obs-studio#13292](https://github.com/obsproject/obs-studio/issues/13292).
- OBS no longer can crash at startup when the scene collection has a
  PipeWire Video Capture Device source. *Linux*, 2026-10-14. Issue
  [#774](https://github.com/obs-rust/obs-studio/issues/774), PR
  [#775](https://github.com/obs-rust/obs-studio/pull/775), upstream
  [obsproject/obs-studio#13971](https://github.com/obsproject/obs-studio/issues/13971).
- Opening the Properties of a PipeWire camera no longer renegotiates the
  running stream, so libcamera cameras no longer drop or freeze. *Linux*,
  2026-10-15. Issue
  [#819](https://github.com/obs-rust/obs-studio/issues/819), PR
  [#820](https://github.com/obs-rust/obs-studio/pull/820), upstream
  [obsproject/obs-studio#10399](https://github.com/obsproject/obs-studio/issues/10399).
- Camera control sliders of a PipeWire camera can be dragged and moved with
  the mouse wheel again. *Linux*, 2026-10-15. Issue
  [#827](https://github.com/obs-rust/obs-studio/issues/827), PR
  [#829](https://github.com/obs-rust/obs-studio/pull/829).
- V4L2 camera controls that the camera refuses (for example manual exposure
  in an automatic mode) are disabled, so other sliders can be dragged again.
  *Linux*, 2026-10-15. Issue
  [#827](https://github.com/obs-rust/obs-studio/issues/827), PR
  [#895](https://github.com/obs-rust/obs-studio/pull/895).
- A new resolution for a running PipeWire camera restarts the stream, so
  libcamera cameras no longer lose the stream. *Linux*, 2026-10-16. Issues
  [#390](https://github.com/obs-rust/obs-studio/issues/390),
  [#933](https://github.com/obs-rust/obs-studio/issues/933), PR
  [#934](https://github.com/obs-rust/obs-studio/pull/934), upstream
  [obsproject/obs-studio#10399](https://github.com/obsproject/obs-studio/issues/10399).
- In the Video Capture Device (PipeWire) source, a manual camera control is
  greyed out while its automatic mode is on, and menu controls such as Auto
  Exposure and Power Line Frequency show again. *Linux*, 2026-10-16. Issue
  [#827](https://github.com/obs-rust/obs-studio/issues/827), PR
  [#1054](https://github.com/obs-rust/obs-studio/pull/1054).
- A Screen Capture (PipeWire) source that is not showing keeps its stream
  paused, so several captures no longer slow down the desktop and OBS.
  *Linux*, 2026-10-16. Issue
  [#1110](https://github.com/obs-rust/obs-studio/issues/1110), PR
  [#1111](https://github.com/obs-rust/obs-studio/pull/1111), upstream
  [obsproject/obs-studio#13836](https://github.com/obsproject/obs-studio/issues/13836).
- A V4L2 camera from a scene collection saved by older OBS runs at the frame
  rate you picked, not at the camera's fastest rate. *Linux*, 2026-10-16.
  Issue [#1117](https://github.com/obs-rust/obs-studio/issues/1117), PR
  [#1118](https://github.com/obs-rust/obs-studio/pull/1118), upstream
  [obsproject/obs-studio#14036](https://github.com/obsproject/obs-studio/issues/14036),
  [obsproject/obs-studio#201](https://github.com/obsproject/obs-studio/issues/201).
- Picking a PipeWire camera format that does not have the picked frame rate
  no longer freezes the picture. *Linux*, 2026-10-16. Issue
  [#1119](https://github.com/obs-rust/obs-studio/issues/1119), PR
  [#1120](https://github.com/obs-rust/obs-studio/pull/1120), upstream
  [obsproject/obs-studio#13783](https://github.com/obsproject/obs-studio/issues/13783).
- The format list of a PipeWire camera source shows each format and size
  once. *Linux*, 2026-10-17. Issue
  [#1068](https://github.com/obs-rust/obs-studio/issues/1068), PR
  [#1217](https://github.com/obs-rust/obs-studio/pull/1217).
- Window Capture has a "List OBS windows" option (off by default) that adds
  OBS's own windows, such as projectors, to the window list so OBS can capture
  itself. *Windows*, 2026-10-24. Issue
  [#2226](https://github.com/obs-rust/obs-studio/issues/2226).
- Screen Capture (XSHM) follows a change of the screen's resolution: the
  capture is resized instead of freezing (screen made smaller) or staying
  cropped to the old size (screen made larger). *Linux*, 2026-10-20. Issue
  [#1494](https://github.com/obs-rust/obs-studio/issues/1494), PR
  [#1495](https://github.com/obs-rust/obs-studio/pull/1495), port of
  [obsproject/obs-studio#12937](https://github.com/obsproject/obs-studio/pull/12937).
- Screen Capture (XSHM) draws the cursor in the right place after the captured
  monitor moves without changing size (a monitor plugged in or rearranged).
  *Linux*, 2026-10-24. Issue
  [#2265](https://github.com/obs-rust/obs-studio/issues/2265).
- The OBS Virtual Camera no longer burns about a millisecond of CPU per frame
  spinning inside the app that uses it (Zoom, Teams, a browser); it hands each
  frame to that app slightly earlier instead. *Windows*, 2026-10-21. Issue
  [#1279](https://github.com/obs-rust/obs-studio/issues/1279), PR
  [#1638](https://github.com/obs-rust/obs-studio/pull/1638).
- A camera or other async source rotated by a multiple of 90 degrees other
  than 0, 90, -90, 180 or 270 (such as -180, -270 or 450, from a camera's
  autorotation or a plugin) is drawn turned in its place, instead of outside
  its box. *All*, pending merge. Issue
  [#2121](https://github.com/obs-rust/obs-studio/issues/2121).
- Turning off Autorotation on a Video Capture Device source takes back the
  rotation it applied, instead of leaving the camera turned until OBS
  restarts or the source is recreated. *Windows*, pending merge. Issue
  [#2137](https://github.com/obs-rust/obs-studio/issues/2137).
- A PipeWire screen or window capture of a stream with the Flipped or
  Flipped180 transform is mirrored left to right again instead of drawn
  upside down: `GS_FLIP_U` on a 2D texture (`gs_draw_sprite()`, and a
  full-size `gs_draw_sprite_subregion()`) flips it on u, not on v, as it did
  before upstream 7b8b51b29. *All*, 2026-10-24. Issue
  [#2124](https://github.com/obs-rust/obs-studio/issues/2124), PR
  [#2135](https://github.com/obs-rust/obs-studio/pull/2135).
- A V4L2 Video Capture Device follows its camera, not a /dev/videoN number:
  a camera replugged or renumbered under another number is captured again,
  and another camera that takes its old number is not captured in its place
  (the log says the source waits for its camera). The source saves the
  camera's name, bus and USB serial number next to the node; two cameras of
  one model without serial numbers are told apart by their USB port.
  *Linux*, pending merge. Issue
  [#2262](https://github.com/obs-rust/obs-studio/issues/2262), upstream
  [obsproject/obs-studio#3824](https://github.com/obsproject/obs-studio/issues/3824),
  [obsproject/obs-studio#4972](https://github.com/obsproject/obs-studio/issues/4972).
- A V4L2 camera that stops working without being unplugged (a firmware
  hang, an error after suspend and resume) no longer freezes its source on
  the last frame: OBS closes the camera and opens it again until it
  delivers frames. *Linux*, pending merge. Issue
  [#2264](https://github.com/obs-rust/obs-studio/issues/2264), upstream
  [obsproject/obs-studio#9434](https://github.com/obsproject/obs-studio/issues/9434).
- The Virtual Camera can be stopped and started again while recording: the
  recording no longer keeps the v4l2loopback device open, and no longer
  holds the VA-API encoder's GPU device either. *Linux*, pending merge.
  Issue [#2278](https://github.com/obs-rust/obs-studio/issues/2278), upstream
  [obsproject/obs-studio#14054](https://github.com/obsproject/obs-studio/issues/14054).

### Audio

- PulseAudio audio input on pipewire-pulse no longer cuts out or stutters
  when audio packets arrive in bursts. *Linux*, 2026-10-11. Issue
  [#190](https://github.com/obs-rust/obs-studio/issues/190), PR
  [#194](https://github.com/obs-rust/obs-studio/pull/194), upstream
  [obsproject/obs-studio#12835](https://github.com/obsproject/obs-studio/issues/12835),
  [obsproject/obs-studio#10808](https://github.com/obsproject/obs-studio/issues/10808).
- On Wayland, push-to-talk and other hold hotkeys stay pressed while the key
  is held and an OBS window has focus. *Linux*, 2026-10-11. Issue
  [#210](https://github.com/obs-rust/obs-studio/issues/210), PR
  [#211](https://github.com/obs-rust/obs-studio/pull/211), upstream
  [obsproject/obs-studio#10701](https://github.com/obsproject/obs-studio/issues/10701).
- VLC Video Source plays 5.1 and 7.1 audio through the correct speakers.
  *All*, 2026-10-11. Issue
  [#251](https://github.com/obs-rust/obs-studio/issues/251), PR
  [#252](https://github.com/obs-rust/obs-studio/pull/252), upstream
  [obsproject/obs-studio#12579](https://github.com/obsproject/obs-studio/issues/12579).
- Audio capture devices on ALSA, JACK and PulseAudio no longer drop out
  every few minutes because their clock is slightly fast or slow. *Linux*,
  2026-10-11. Issue
  [#270](https://github.com/obs-rust/obs-studio/issues/270), PR
  [#271](https://github.com/obs-rust/obs-studio/pull/271), upstream
  [obsproject/obs-studio#4600](https://github.com/obsproject/obs-studio/issues/4600).
- Scene transitions fade the audio again when an output uses Rescale Output.
  *All*, 2026-10-11. Issue
  [#272](https://github.com/obs-rust/obs-studio/issues/272), PR
  [#273](https://github.com/obs-rust/obs-studio/pull/273), upstream
  [obsproject/obs-studio#12912](https://github.com/obsproject/obs-studio/issues/12912).
- The audio monitoring device you choose stays in use when the system
  default output changes (PulseAudio and PipeWire). *Linux*, 2026-10-11.
  Issue [#274](https://github.com/obs-rust/obs-studio/issues/274), PR
  [#275](https://github.com/obs-rust/obs-studio/pull/275), upstream
  [obsproject/obs-studio#10342](https://github.com/obsproject/obs-studio/issues/10342).
- A Media Source keeps its audio sync offset when playback starts or
  restarts. *All*, 2026-10-11. Issue
  [#286](https://github.com/obs-rust/obs-studio/issues/286), PR
  [#287](https://github.com/obs-rust/obs-studio/pull/287), upstream
  [obsproject/obs-studio#11640](https://github.com/obsproject/obs-studio/issues/11640).
- A VLC Video Source playlist no longer cuts the last second of each audio
  track. *All*, 2026-10-12. Issue
  [#301](https://github.com/obs-rust/obs-studio/issues/301), PR
  [#302](https://github.com/obs-rust/obs-studio/pull/302), upstream
  [obsproject/obs-studio#9345](https://github.com/obsproject/obs-studio/issues/9345).
- An audio sync offset changed from 0 on a running source now takes effect,
  and a saved offset applies when a scene collection loads. *All*,
  2026-10-12. Issue
  [#333](https://github.com/obs-rust/obs-studio/issues/333), PR
  [#334](https://github.com/obs-rust/obs-studio/pull/334), upstream
  [obsproject/obs-studio#7912](https://github.com/obsproject/obs-studio/issues/7912).
- An audio input on PulseAudio or PipeWire records again after its stream
  dies, without a settings change or restart. *Linux*, 2026-10-12. Issues
  [#377](https://github.com/obs-rust/obs-studio/issues/377),
  [#1029](https://github.com/obs-rust/obs-studio/issues/1029),
  [#1056](https://github.com/obs-rust/obs-studio/issues/1056), PRs
  [#378](https://github.com/obs-rust/obs-studio/pull/378),
  [#1031](https://github.com/obs-rust/obs-studio/pull/1031),
  [#1057](https://github.com/obs-rust/obs-studio/pull/1057), upstream
  [obsproject/obs-studio#10072](https://github.com/obsproject/obs-studio/issues/10072).
- The VLC Video Source sends its audio as it plays, so volume meters and
  audio callbacks no longer run up to 2 s early or reset every second.
  *All*, 2026-10-12. Issue
  [#404](https://github.com/obs-rust/obs-studio/issues/404), PR
  [#405](https://github.com/obs-rust/obs-studio/pull/405), upstream
  [obsproject/obs-studio#2406](https://github.com/obsproject/obs-studio/issues/2406).
- A Video Delay (Async) filter of more than 2 s no longer breaks up the
  audio of the source into noise. *All*, 2026-10-13. Issue
  [#521](https://github.com/obs-rust/obs-studio/issues/521), PR
  [#522](https://github.com/obs-rust/obs-studio/pull/522), upstream
  [obsproject/obs-studio#13830](https://github.com/obsproject/obs-studio/issues/13830).
- Two Audio Input Capture sources on the same device both record cleanly
  when PipeWire runs at a large quantum, and the second one is no longer
  mostly silent. *Linux*, 2026-10-13. Issue
  [#531](https://github.com/obs-rust/obs-studio/issues/531), PR
  [#532](https://github.com/obs-rust/obs-studio/pull/532), upstream
  [obsproject/obs-studio#10808](https://github.com/obsproject/obs-studio/issues/10808).
- A restarted Media Source with a sample rate different from OBS no longer
  starts with a click from the end of its previous playback. *All*,
  2026-10-13. Issue
  [#545](https://github.com/obs-rust/obs-studio/issues/545), PR
  [#546](https://github.com/obs-rust/obs-studio/pull/546), upstream
  [obsproject/obs-studio#7063](https://github.com/obsproject/obs-studio/issues/7063).
- In Studio Mode, the Audio Mixer marks audio sources in groups and nested
  scenes of the previewed scene as Preview. *All*, 2026-10-13. Issue
  [#602](https://github.com/obs-rust/obs-studio/issues/602), PR
  [#603](https://github.com/obs-rust/obs-studio/pull/603), upstream
  [obsproject/obs-studio#13297](https://github.com/obsproject/obs-studio/issues/13297).
- Fade to Color with a switch point of 0% or 100% no longer produces broken
  (NaN) audio. *All*, 2026-10-14. Issue
  [#488](https://github.com/obs-rust/obs-studio/issues/488), PR
  [#687](https://github.com/obs-rust/obs-studio/pull/687).
- When a PulseAudio monitor reset fails, OBS keeps the working monitor
  instead of leaving freed state behind. *Linux*, 2026-10-14. Issues
  [#486](https://github.com/obs-rust/obs-studio/issues/486),
  [#686](https://github.com/obs-rust/obs-studio/issues/686), PR
  [#713](https://github.com/obs-rust/obs-studio/pull/713).
- Removing an ALSA Audio Capture Device source whose device is missing no
  longer crashes or hangs OBS. *Linux*, 2026-10-14. Issue
  [#770](https://github.com/obs-rust/obs-studio/issues/770), PR
  [#771](https://github.com/obs-rust/obs-studio/pull/771), upstream
  [obsproject/obs-studio#9322](https://github.com/obsproject/obs-studio/issues/9322).
- Audio monitoring on PulseAudio and PipeWire catches up after a playback
  stall or drift instead of staying seconds behind. *Linux*, 2026-10-15.
  Issue [#778](https://github.com/obs-rust/obs-studio/issues/778), PR
  [#779](https://github.com/obs-rust/obs-studio/pull/779), upstream
  [obsproject/obs-studio#10548](https://github.com/obsproject/obs-studio/issues/10548),
  [obsproject/obs-studio#3525](https://github.com/obsproject/obs-studio/issues/3525),
  [obsproject/obs-studio#4531](https://github.com/obsproject/obs-studio/issues/4531).
- The Expander filter at a 1:1 ratio no longer turns down the audio that
  follows silence. *All*, 2026-10-16. PR
  [#995](https://github.com/obs-rust/obs-studio/pull/995).
- A disconnected macOS audio device no longer makes OBS retry CoreAudio
  every 300 ms or 2 s forever: retries back off to every 30 s and restart
  quickly when a device is plugged in or the default device changes.
  *macOS*, 2026-10-17. Issue
  [#1358](https://github.com/obs-rust/obs-studio/issues/1358), PR
  [#1370](https://github.com/obs-rust/obs-studio/pull/1370).

- Audio timestamp differences smaller than one sample no longer leave an
  invalid interval in the mixer. *All*, 2026-10-18. Issue
  [#838](https://github.com/obs-rust/obs-studio/issues/838), PR
  [#1333](https://github.com/obs-rust/obs-studio/pull/1333).
- Audio monitoring above 100% volume no longer distorts on PulseAudio and
  PipeWire outputs that play 16-bit, 32-bit or 8-bit integer audio: loud
  samples clip instead of wrapping around. *Linux*, 2026-10-21. Issue
  [#1697](https://github.com/obs-rust/obs-studio/issues/1697), PR
  [#1709](https://github.com/obs-rust/obs-studio/pull/1709).
- A push-to-talk or push-to-mute delay too large to count in nanoseconds
  no longer wraps around and ends the moment the key is released; it holds
  for as long as it can. A negative delay in a scene collection loads as no
  delay. An audio balance outside full left to full right, from a scene
  collection, a plugin or a script, is clamped to that range (NaN is
  centred) instead of inverting or overdriving a channel. *All*, pending
  merge. Issue
  [#2051](https://github.com/obs-rust/obs-studio/issues/2051), PR
  [#2058](https://github.com/obs-rust/obs-studio/pull/2058).
- A source volume that is not a number, negative or infinite, from a scene
  collection, a plugin or a script, no longer silences or corrupts every
  audio track the source is in: NaN is read as 100%, a negative volume as
  0% and an infinite one as 2000% (+26 dB), the most the volume controls
  allow. *All*, pending merge. Issue
  [#2108](https://github.com/obs-rust/obs-studio/issues/2108).
- Desktop Audio set to "Default" on PulseAudio no longer goes silent when the
  default output changes, and no longer records the microphone when the
  default output is unplugged: it follows the new default output. Saving its
  properties unchanged no longer restarts it. *Linux*, pending merge. Issue
  [#2260](https://github.com/obs-rust/obs-studio/issues/2260), PR
  [#2271](https://github.com/obs-rust/obs-studio/pull/2271), upstream
  [obsproject/obs-studio#4828](https://github.com/obsproject/obs-studio/issues/4828),
  [obsproject/obs-studio#3211](https://github.com/obsproject/obs-studio/issues/3211).
- A browser source playing sound through WebAudio (alert boxes and the like)
  no longer goes silent for good when Chromium's audio service process dies;
  its sound comes back about 3 s later, without a page refresh or toggling
  "Control audio via OBS". A page that paused its own audio stays paused.
  *All*, pending merge. Issue
  [#2328](https://github.com/obs-rust/obs-studio/issues/2328), upstream
  [obsproject/obs-studio#10586](https://github.com/obsproject/obs-studio/issues/10586).
- A browser source with "Control audio via OBS" no longer raises the audio
  buffering of every source for good (up to 960 ms of added latency) when
  its audio arrives late under load. Audio that arrives too late to mix is
  dropped and logged instead: during such a stall the browser source is
  silent for about the stall's length (up to about a second), and other
  sources keep their latency. *All*, pending merge. Issue
  [#2335](https://github.com/obs-rust/obs-studio/issues/2335).

### Encoding, streaming and recording

- A recording no longer fails with "incomplete UTF-8 string" when a long
  filename cuts a multibyte character at 255 bytes. *All*, 2026-10-09. Issue
  [#21](https://github.com/obs-rust/obs-studio/issues/21), PRs
  [#23](https://github.com/obs-rust/obs-studio/pull/23),
  [#26](https://github.com/obs-rust/obs-studio/pull/26), upstream
  [obsproject/obs-studio#11211](https://github.com/obsproject/obs-studio/issues/11211).
- The SVT-AV1 encoder now uses CBR rate control when CBR is selected, and
  invalid SVT-AV1 options stop the encoder with an error. *All*, 2026-10-09.
  Issue [#24](https://github.com/obs-rust/obs-studio/issues/24), PR
  [#63](https://github.com/obs-rust/obs-studio/pull/63), upstream
  [obsproject/obs-studio#13849](https://github.com/obsproject/obs-studio/issues/13849).
- OBS no longer crashes when an RTMP server returns a connect error without
  a description. *All*, 2026-10-09. Issue
  [#57](https://github.com/obs-rust/obs-studio/issues/57), PR
  [#64](https://github.com/obs-rust/obs-studio/pull/64), upstream
  [obsproject/obs-studio#13144](https://github.com/obsproject/obs-studio/issues/13144).
- OBS no longer reads past the end of malformed or unusual AV1 packets, and
  AV1 headers with an extension byte and no size field get the correct size.
  *All*, 2026-10-10. PR
  [#103](https://github.com/obs-rust/obs-studio/pull/103).
- Streaming over RTMP or recording to FLV no longer crashes on an empty
  packet on a secondary audio track. *All*, 2026-10-10. Issue
  [#101](https://github.com/obs-rust/obs-studio/issues/101), PR
  [#110](https://github.com/obs-rust/obs-studio/pull/110).
- FFmpeg VAAPI encoders start on the default device, with a log message,
  when the saved VAAPI device is not usable on this machine. *Linux*,
  2026-10-11. Issue
  [#197](https://github.com/obs-rust/obs-studio/issues/197), PR
  [#198](https://github.com/obs-rust/obs-studio/pull/198), upstream
  [obsproject/obs-studio#13773](https://github.com/obsproject/obs-studio/issues/13773).
- In Advanced output mode, recording and split file names have characters
  such as : replaced, the same as in Simple mode. *All*, 2026-10-11. Issue
  [#199](https://github.com/obs-rust/obs-studio/issues/199), PR
  [#200](https://github.com/obs-rust/obs-studio/pull/200), upstream
  [obsproject/obs-studio#13786](https://github.com/obsproject/obs-studio/issues/13786).
- Starting a recording or stream with a VAAPI texture encoder and a color
  format other than NV12 or P010 no longer crashes OBS; it uses the CPU
  VAAPI encoder instead. *Linux*, 2026-10-11. Issue
  [#206](https://github.com/obs-rust/obs-studio/issues/206), PR
  [#207](https://github.com/obs-rust/obs-studio/pull/207), upstream
  [obsproject/obs-studio#12731](https://github.com/obsproject/obs-studio/issues/12731).
- Recording with the FFmpeg VAAPI H.264 or HEVC encoder no longer crashes at
  the start when the first packet holds only headers. *Linux*, 2026-10-11.
  Issue [#216](https://github.com/obs-rust/obs-studio/issues/216), PR
  [#218](https://github.com/obs-rust/obs-studio/pull/218), upstream
  [obsproject/obs-studio#13316](https://github.com/obsproject/obs-studio/issues/13316).
- The replay buffer stops while a shared recording is paused, and OBS no
  longer hangs on exit in that case. *All*, 2026-10-11. Issue
  [#223](https://github.com/obs-rust/obs-studio/issues/223), PR
  [#224](https://github.com/obs-rust/obs-studio/pull/224), upstream
  [obsproject/obs-studio#7249](https://github.com/obsproject/obs-studio/issues/7249).
- WHIP streams at 23.976 or 59.94 fps no longer drift out of audio/video
  sync over time. *All*, 2026-10-11. Issue
  [#225](https://github.com/obs-rust/obs-studio/issues/225), PR
  [#226](https://github.com/obs-rust/obs-studio/pull/226), upstream
  [obsproject/obs-studio#13970](https://github.com/obsproject/obs-studio/issues/13970).
- When a stream fails to start, OBS shuts down its encoders and releases
  hardware encoder sessions such as NVENC. *All*, 2026-10-11. Issue
  [#236](https://github.com/obs-rust/obs-studio/issues/236), PR
  [#237](https://github.com/obs-rust/obs-studio/pull/237), upstream
  [obsproject/obs-studio#10651](https://github.com/obsproject/obs-studio/issues/10651).
- When the recording muxer process stops during a recording, OBS now sees
  the write failure instead of discarding all later data in silence. *Linux,
  macOS*, 2026-10-11. Issue
  [#264](https://github.com/obs-rust/obs-studio/issues/264), PR
  [#265](https://github.com/obs-rust/obs-studio/pull/265).
- A mono Opus audio track set above 256 kbps now starts (at 256 kbps, with a
  log warning) instead of failing. *All*, 2026-10-12. Issue
  [#295](https://github.com/obs-rust/obs-studio/issues/295), PR
  [#296](https://github.com/obs-rust/obs-studio/pull/296), upstream
  [obsproject/obs-studio#12753](https://github.com/obsproject/obs-studio/issues/12753).
- An encoder error during a recording no longer freezes OBS or leaves an
  output that shares the encoder unable to start. *All*, 2026-10-12. Issue
  [#298](https://github.com/obs-rust/obs-studio/issues/298), PR
  [#299](https://github.com/obs-rust/obs-studio/pull/299), upstream
  [obsproject/obs-studio#9946](https://github.com/obsproject/obs-studio/issues/9946).
- Recordings that start in the same second no longer write to the same file
  and overwrite each other. *All*, 2026-10-12. Issue
  [#308](https://github.com/obs-rust/obs-studio/issues/308), PR
  [#309](https://github.com/obs-rust/obs-studio/pull/309), upstream
  [obsproject/obs-studio#10750](https://github.com/obsproject/obs-studio/issues/10750).
- A custom FFmpeg recording to WebM/VP9 with the BGRA format now starts and
  keeps its alpha channel. *All*, 2026-10-12. Issue
  [#310](https://github.com/obs-rust/obs-studio/issues/310), PR
  [#311](https://github.com/obs-rust/obs-studio/pull/311), upstream
  [obsproject/obs-studio#13785](https://github.com/obsproject/obs-studio/issues/13785).
- With Rescale Output on, Stats counts frames skipped due to encoding lag
  and the status bar warns about encoder overload. *All*, 2026-10-12. Issue
  [#336](https://github.com/obs-rust/obs-studio/issues/336), PR
  [#337](https://github.com/obs-rust/obs-studio/pull/337), upstream
  [obsproject/obs-studio#11078](https://github.com/obsproject/obs-studio/issues/11078).
- Caption text longer than 128 bytes, also from obs-websocket
  SendStreamCaption, is wrapped and paged instead of cut off. *All*,
  2026-10-12. PR [#341](https://github.com/obs-rust/obs-studio/pull/341).
- Stopping a Custom Output (FFmpeg) recording no longer crashes or freezes
  OBS, for example with libvpx. *All*, 2026-10-12. Issue
  [#354](https://github.com/obs-rust/obs-studio/issues/354), PR
  [#355](https://github.com/obs-rust/obs-studio/pull/355), upstream
  [obsproject/obs-studio#7536](https://github.com/obsproject/obs-studio/issues/7536).
- Remuxing an MKV recording to MP4 or MOV keeps the audio sample-accurate,
  so it no longer crackles in QuickTime and Final Cut Pro. *All*,
  2026-10-12. Issue
  [#359](https://github.com/obs-rust/obs-studio/issues/359), PR
  [#360](https://github.com/obs-rust/obs-studio/pull/360), upstream
  [obsproject/obs-studio#13249](https://github.com/obsproject/obs-studio/issues/13249).
- In Advanced output mode, "Ignore streaming service setting
  recommendations" keeps your encoder settings, such as CQP or CRF. *All*,
  2026-10-12. Issue
  [#368](https://github.com/obs-rust/obs-studio/issues/368), PR
  [#369](https://github.com/obs-rust/obs-studio/pull/369), upstream
  [obsproject/obs-studio#4125](https://github.com/obsproject/obs-studio/issues/4125).
- FFmpeg output recordings that convert the pixel format no longer leak
  memory for each recording. *All*, 2026-10-12. PR
  [#375](https://github.com/obs-rust/obs-studio/pull/375).
- A caption character that EIA-608 cannot encode (for example CJK or emoji)
  becomes a gap, and the rest of the caption row is still sent. *All*,
  2026-10-12. Issue
  [#376](https://github.com/obs-rust/obs-studio/issues/376), PR
  [#412](https://github.com/obs-rust/obs-studio/pull/412).
- Hybrid MP4 recordings with 5.1 audio play with sound in VLC 3. *All*,
  2026-10-13. Issue
  [#494](https://github.com/obs-rust/obs-studio/issues/494), PR
  [#495](https://github.com/obs-rust/obs-studio/pull/495).
- A recording with Custom Output (FFmpeg) set to "Output to URL" is no
  longer stopped for low disk space. *All*, 2026-10-13. Issue
  [#526](https://github.com/obs-rust/obs-studio/issues/526), PR
  [#527](https://github.com/obs-rust/obs-studio/pull/527), upstream
  [obsproject/obs-studio#12952](https://github.com/obsproject/obs-studio/issues/12952).
- With dynamic bitrate on, a video bitrate that you change while live stays
  in effect after network congestion. *All*, 2026-10-13. Issue
  [#574](https://github.com/obs-rust/obs-studio/issues/574), PR
  [#575](https://github.com/obs-rust/obs-studio/pull/575), upstream
  [obsproject/obs-studio#12235](https://github.com/obsproject/obs-studio/issues/12235).
- A source with Subtract blending no longer shows black in a rescaled
  recording or stream. *All*, 2026-10-13. Issue
  [#580](https://github.com/obs-rust/obs-studio/issues/580), PR
  [#581](https://github.com/obs-rust/obs-studio/pull/581), upstream
  [obsproject/obs-studio#10746](https://github.com/obsproject/obs-studio/issues/10746).
- A saved replay uses the current filename settings, not the settings from
  when the replay buffer started. *All*, 2026-10-13. Issue
  [#583](https://github.com/obs-rust/obs-studio/issues/583), PR
  [#584](https://github.com/obs-rust/obs-studio/pull/584), upstream
  [obsproject/obs-studio#10923](https://github.com/obsproject/obs-studio/issues/10923).
- A recording started while the replay buffer runs starts at once, instead
  of waiting for the next keyframe or leaving an empty file. *All*,
  2026-10-13. Issues
  [#593](https://github.com/obs-rust/obs-studio/issues/593),
  [#597](https://github.com/obs-rust/obs-studio/issues/597), PRs
  [#594](https://github.com/obs-rust/obs-studio/pull/594),
  [#598](https://github.com/obs-rust/obs-studio/pull/598), upstream
  [obsproject/obs-studio#12069](https://github.com/obsproject/obs-studio/issues/12069).
- Tools > Output Timer shows Start again after its timer stopped the output
  while the dialog was closed. *All*, 2026-10-13. Issue
  [#600](https://github.com/obs-rust/obs-studio/issues/600), PR
  [#601](https://github.com/obs-rust/obs-studio/pull/601), upstream
  [obsproject/obs-studio#10900](https://github.com/obsproject/obs-studio/issues/10900).
- Restarting the same output no longer makes its interleaving batch limit
  grow each time. *All*, 2026-10-14. Issues
  [#487](https://github.com/obs-rust/obs-studio/issues/487),
  [#686](https://github.com/obs-rust/obs-studio/issues/686), PR
  [#704](https://github.com/obs-rust/obs-studio/pull/704).
- A RIST stream that fails to open no longer can crash OBS by freeing its
  credentials twice. *All*, 2026-10-14. Issues
  [#482](https://github.com/obs-rust/obs-studio/issues/482),
  [#686](https://github.com/obs-rust/obs-studio/issues/686), PR
  [#705](https://github.com/obs-rust/obs-studio/pull/705).
- A GPU encoder that starts while the last one on the same video mix stops
  keeps getting frames; before, it could stop receiving frames. *All*,
  2026-10-14. PR [#736](https://github.com/obs-rust/obs-studio/pull/736).
- Quitting OBS while it records stops the recording correctly, so the end of
  the recording is not lost. *All*, 2026-10-15. Issue
  [#780](https://github.com/obs-rust/obs-studio/issues/780), PR
  [#781](https://github.com/obs-rust/obs-studio/pull/781).
- Start Recording no longer crashes OBS after an earlier attempt failed with
  "Unable to write to". *All*, 2026-10-15. Issue
  [#802](https://github.com/obs-rust/obs-studio/issues/802), PR
  [#803](https://github.com/obs-rust/obs-studio/pull/803), upstream
  [obsproject/obs-studio#11940](https://github.com/obsproject/obs-studio/issues/11940).
- When the stream server name cannot be resolved, the log shows the real
  lookup error instead of "Success". *All*, 2026-10-15. Issue
  [#903](https://github.com/obs-rust/obs-studio/issues/903), PR
  [#918](https://github.com/obs-rust/obs-studio/pull/918).
- With bitrate limiting on, the VideoToolbox encoder on macOS gets the
  configured limit values; before, it got wrong values. *macOS*, 2026-10-16.
  PR [#996](https://github.com/obs-rust/obs-studio/pull/996).
- An SRT stream that fails to connect no longer disconnects the other SRT
  streams and Media Sources in OBS. *All*, 2026-10-16. Issue
  [#1101](https://github.com/obs-rust/obs-studio/issues/1101), PR
  [#1102](https://github.com/obs-rust/obs-studio/pull/1102), upstream
  [obsproject/obs-studio#12361](https://github.com/obsproject/obs-studio/issues/12361).
- A recording or stream through the Custom FFmpeg Output keeps its last
  second of video and audio when it stops. *All*, 2026-10-16. Issue
  [#1124](https://github.com/obs-rust/obs-studio/issues/1124), PR
  [#1125](https://github.com/obs-rust/obs-studio/pull/1125).
- With the replay buffer on, a hotkey change in Settings no longer makes OBS
  log "Failed reading json string" each time the replay buffer is created.
  *All*, 2026-10-17. Issue
  [#1137](https://github.com/obs-rust/obs-studio/issues/1137), PR
  [#1138](https://github.com/obs-rust/obs-studio/pull/1138), upstream
  [obsproject/obs-studio#14014](https://github.com/obsproject/obs-studio/issues/14014).
- Starting a Twitch, Amazon IVS or Dacast stream while their ingest server
  list cannot be downloaded no longer stalls 3 s before connecting (5 s in
  the Auto-Configuration Wizard): the wait ends as soon as the download
  fails. *All*, 2026-10-17. Issue
  [#1347](https://github.com/obs-rust/obs-studio/issues/1347), PR
  [#1353](https://github.com/obs-rust/obs-studio/pull/1353).
- Stop Streaming stops an SRT listener stream that is waiting for its caller
  to come back, and OBS quits while a listener waits; before, the stream
  never stopped and OBS never finished quitting. *All*, 2026-10-18. Issue
  [#1421](https://github.com/obs-rust/obs-studio/issues/1421), PR
  [#1422](https://github.com/obs-rust/obs-studio/pull/1422), upstream
  [obsproject/obs-studio#12250](https://github.com/obsproject/obs-studio/issues/12250).
- OBS no longer crashes when an SRT or RIST stream reconnects after its
  connection dropped; before, a packet left from the dropped connection could
  reach the new connection before its header and kill OBS. *All*,
  2026-10-19. Issue
  [#1423](https://github.com/obs-rust/obs-studio/issues/1423), PR
  [#1475](https://github.com/obs-rust/obs-studio/pull/1475).
- Automatic file splitting by time splits Hybrid MP4 recordings again: the
  next file starts at the first keyframe after the split time, as for MKV
  and MP4. Before, the recording stayed in one file unless a keyframe fell
  within 1 ms of the split time. *All*, 2026-10-19. Issue
  [#1476](https://github.com/obs-rust/obs-studio/issues/1476), PR
  [#1477](https://github.com/obs-rust/obs-studio/pull/1477), upstream
  [obsproject/obs-studio#14051](https://github.com/obsproject/obs-studio/issues/14051),
  port of
  [obsproject/obs-studio#14052](https://github.com/obsproject/obs-studio/pull/14052).
- Settings > Output (Advanced) with a profile that names a video encoder
  OBS does not have (a profile copied from a machine with an AMD encoder,
  for example) selects a fallback encoder, x264 for most, and shows its
  settings. Before, the encoder settings read "No properties available",
  the streaming list showed an encoder that was not in use, the recording
  list was empty and OK only said "No Encoder Selected". The profile changes
  only when the output settings are saved. *All*, 2026-10-20. Issue
  [#1490](https://github.com/obs-rust/obs-studio/issues/1490), PR
  [#1491](https://github.com/obs-rust/obs-studio/pull/1491), upstream
  [obsproject/obs-studio#3531](https://github.com/obsproject/obs-studio/issues/3531).
- With Enhanced Broadcasting (multitrack video), the Broadcast Performance
  Metrics of a frame whose time falls on a whole second carry that time.
  Before, the time read 900 ms early (`…:32.1000Z` for `…:33.000Z`).
  *All*, 2026-10-20. Issue
  [#1505](https://github.com/obs-rust/obs-studio/issues/1505), PR
  [#1508](https://github.com/obs-rust/obs-studio/pull/1508).
- A plugin or libobs-based tool that builds a file name from a format with
  `%FPS`, `%CRES`, `%ORES` or `%VF` before video is set up gets `0`, `0x0`
  and `None` there. Before, those were formatted from uninitialized memory
  (names like `2880154539x2880154539`). *All*, 2026-10-20. Issue
  [#1509](https://github.com/obs-rust/obs-studio/issues/1509), PRs
  [#1562](https://github.com/obs-rust/obs-studio/pull/1562) and
  [#1565](https://github.com/obs-rust/obs-studio/pull/1565).
- Streaming over RTMP no longer leaks memory each time the server drops the
  connection before the first video frame is sent, or sends a message OBS
  can't read before it drops the connection. *All*, 2026-10-20. Issue
  [#1573](https://github.com/obs-rust/obs-studio/issues/1573), PR
  [#1574](https://github.com/obs-rust/obs-studio/pull/1574), upstream
  [obsproject/obs-studio#9972](https://github.com/obsproject/obs-studio/issues/9972).
  [#1509](https://github.com/obs-rust/obs-studio/issues/1509), PRs
  [#1562](https://github.com/obs-rust/obs-studio/pull/1562) and
  [#1565](https://github.com/obs-rust/obs-studio/pull/1565).
- With a stream delay, a stream started again after it stopped on its own
  (the connection dropped with Automatically Reconnect off, or an encoder
  error) shows as streaming again. Before, OBS and obs-websocket never
  reported it as started, and the delayed output passed packets on before
  it had connected. *All*, 2026-10-20. Issue
  [#1571](https://github.com/obs-rust/obs-studio/issues/1571), PR
  [#1572](https://github.com/obs-rust/obs-studio/pull/1572).
- An output whose only video encoder is not in the first slot (a plugin
  that sets one with `obs_output_set_video_encoder2()` and an index above 0)
  starts on its first keyframe, with its timestamps starting at 0. Before,
  the first keyframe was never sent and the output started on a frame that
  is not a keyframe. *All*, 2026-10-21. Issue
  [#1611](https://github.com/obs-rust/obs-studio/issues/1611), PR
  [#1612](https://github.com/obs-rust/obs-studio/pull/1612).
- Captions sent to an output whose video codec cannot carry them (not
  H.264, HEVC or AV1, for example ProRes or a plugin's codec) no longer
  leak a copy of a video frame for each caption. *All*, 2026-10-22. Issue
  [#1777](https://github.com/obs-rust/obs-studio/issues/1777), PR
  [#1778](https://github.com/obs-rust/obs-studio/pull/1778).
- A plugin or script that adds an encoder region of interest (ROI) that is
  upside down (bottom above top, or right left of left) or has a NaN
  priority gets it refused, as a too small one is; before, it was stored and
  passed to the encoder's quality map. An ROI edge that an upscaled
  encoder scales past the largest value an edge can hold gets that value
  instead of an undefined one, and a NULL encoder no longer crashes OBS. *All*, pending merge. Issue
  [#1959](https://github.com/obs-rust/obs-studio/issues/1959), PR
  [#1964](https://github.com/obs-rust/obs-studio/pull/1964).
- Changing the bitrate of a QSV encoder while streaming no longer races the
  other QSV encoders: the reset now takes the lock they share. A device that
  stays busy or stops finishing frames now stops the output with an error
  instead of leaving it connected at 0 kbps. Live bitrate changes now also
  apply in VBR and in CBR with "low" or "normal" latency, and settings that
  leave the bitrate alone no longer reset the encoder. *Linux, Windows*,
  pending merge. Issue
  [#2291](https://github.com/obs-rust/obs-studio/issues/2291), upstream
  [obsproject/obs-studio#13228](https://github.com/obsproject/obs-studio/issues/13228).

### Sources and filters

- The color grade filter loads 1D .cube LUTs correctly, and a .cube file
  with a very large size no longer crashes OBS or writes out of bounds.
  *All*, 2026-10-10. Issue
  [#94](https://github.com/obs-rust/obs-studio/issues/94), PR
  [#113](https://github.com/obs-rust/obs-studio/pull/113).
- Text (FreeType 2) sources read UTF-16 text files correctly, without
  garbage characters, crashes or a stray byte-order mark. *All*, 2026-10-11.
  Issue [#95](https://github.com/obs-rust/obs-studio/issues/95), PR
  [#121](https://github.com/obs-rust/obs-studio/pull/121).
- A Media Source playing an RTSP camera that goes silent now times out after
  about 10 seconds and reconnects when the camera comes back. *All*,
  2026-10-11. Issue
  [#189](https://github.com/obs-rust/obs-studio/issues/189), PR
  [#193](https://github.com/obs-rust/obs-studio/pull/193), upstream
  [obsproject/obs-studio#12958](https://github.com/obsproject/obs-studio/issues/12958).
- A Text (FreeType 2) source whose text changes often no longer stops
  drawing new letters after a while. *All*, 2026-10-11. Issue
  [#195](https://github.com/obs-rust/obs-studio/issues/195), PR
  [#196](https://github.com/obs-rust/obs-studio/pull/196), upstream
  [obsproject/obs-studio#11819](https://github.com/obsproject/obs-studio/issues/11819).
- A Media Source that restarts when it becomes active no longer flashes its
  old last frame and shows its first frame at once. *All*, 2026-10-11.
  Issues [#249](https://github.com/obs-rust/obs-studio/issues/249),
  [#321](https://github.com/obs-rust/obs-studio/issues/321), PRs
  [#250](https://github.com/obs-rust/obs-studio/pull/250),
  [#322](https://github.com/obs-rust/obs-studio/pull/322), upstream
  [obsproject/obs-studio#4667](https://github.com/obsproject/obs-studio/issues/4667),
  [obsproject/obs-studio#10488](https://github.com/obsproject/obs-studio/issues/10488).
- A Text (FreeType 2) source that reads from a file shows the whole file,
  also when the file contains NUL bytes. *All*, 2026-10-12. Issue
  [#292](https://github.com/obs-rust/obs-studio/issues/292), PR
  [#293](https://github.com/obs-rust/obs-studio/pull/293), upstream
  [obsproject/obs-studio#7595](https://github.com/obsproject/obs-studio/issues/7595).
- A Media Source that plays once (Loop off), such as a GIF, now shows its
  last frames. *All*, 2026-10-12. Issue
  [#306](https://github.com/obs-rust/obs-studio/issues/306), PR
  [#307](https://github.com/obs-rust/obs-studio/pull/307), upstream
  [obsproject/obs-studio#10425](https://github.com/obsproject/obs-studio/issues/10425).
- Seeking in an audio file with embedded cover art continues from the new
  position instead of ending playback. *All*, 2026-10-12. Issue
  [#312](https://github.com/obs-rust/obs-studio/issues/312), PR
  [#313](https://github.com/obs-rust/obs-studio/pull/313), upstream
  [obsproject/obs-studio#11195](https://github.com/obsproject/obs-studio/issues/11195).
- Changing the audio track, subtitle track or network caching of a VLC Video
  Source takes effect every time. *All*, 2026-10-12. Issue
  [#314](https://github.com/obs-rust/obs-studio/issues/314), PR
  [#315](https://github.com/obs-rust/obs-studio/pull/315), upstream
  [obsproject/obs-studio#8346](https://github.com/obsproject/obs-studio/issues/8346).
- Yadif 2x deinterlacing shows bottom-field-first video in the correct
  order. *All*, 2026-10-12. Issue
  [#316](https://github.com/obs-rust/obs-studio/issues/316), PR
  [#317](https://github.com/obs-rust/obs-studio/pull/317), upstream
  [obsproject/obs-studio#6061](https://github.com/obsproject/obs-studio/issues/6061).
- Scale filtering works on rotated scene items instead of aliasing or making
  them transparent. *All*, 2026-10-12. Issue
  [#371](https://github.com/obs-rust/obs-studio/issues/371), PR
  [#372](https://github.com/obs-rust/obs-studio/pull/372), upstream
  [obsproject/obs-studio#12625](https://github.com/obsproject/obs-studio/issues/12625).
- A Stinger transition plays its whole clip when the clip's header gives a
  short or no duration. *All*, 2026-10-12. Issue
  [#373](https://github.com/obs-rust/obs-studio/issues/373), PR
  [#374](https://github.com/obs-rust/obs-studio/pull/374), upstream
  [obsproject/obs-studio#9755](https://github.com/obsproject/obs-studio/issues/9755).
- Double-rate deinterlacing (Yadif 2x, Linear 2x, Discard 2x) shows every
  field when the source field rate is not the canvas rate, so 50i on a 60
  fps canvas is no longer choppy. *All*, 2026-10-12. Issue
  [#394](https://github.com/obs-rust/obs-studio/issues/394), PR
  [#395](https://github.com/obs-rust/obs-studio/pull/395), upstream
  [obsproject/obs-studio#3402](https://github.com/obsproject/obs-studio/issues/3402).
- A FreeType 2 Text source set to "Read from file" shows a file that is
  created after the source is loaded, without a restart. *All*, 2026-10-12.
  Issue [#396](https://github.com/obs-rust/obs-studio/issues/396), PR
  [#397](https://github.com/obs-rust/obs-studio/pull/397), upstream
  [obsproject/obs-studio#3196](https://github.com/obsproject/obs-studio/issues/3196).
- A Media Source no longer leaks memory each time a read fails, for example
  on a network timeout or at the end of a file. *All*, 2026-10-13. Issue
  [#463](https://github.com/obs-rust/obs-studio/issues/463), PR
  [#466](https://github.com/obs-rust/obs-studio/pull/466).
- A Media Source seek (seek bar or obs-websocket SetMediaInputCursor) goes
  to the requested time, not to the keyframe before it. *All*, 2026-10-13.
  Issue [#478](https://github.com/obs-rust/obs-studio/issues/478), PR
  [#479](https://github.com/obs-rust/obs-studio/pull/479), upstream
  [obsproject/obs-websocket#1190](https://github.com/obsproject/obs-websocket/issues/1190).
- Play after Stop or after the end of the file starts a Media Source again
  from the beginning, from the hotkey or obs-websocket
  TriggerMediaInputAction. *All*, 2026-10-13. Issue
  [#504](https://github.com/obs-rust/obs-studio/issues/504), PR
  [#505](https://github.com/obs-rust/obs-studio/pull/505), upstream
  [obsproject/obs-websocket#1046](https://github.com/obsproject/obs-websocket/issues/1046).
- A Media Source follows a stream whose frame size or pixel format changes,
  and the picture no longer freezes or becomes garbled. *All*, 2026-10-13.
  Issue [#517](https://github.com/obs-rust/obs-studio/issues/517), PR
  [#518](https://github.com/obs-rust/obs-studio/pull/518), upstream
  [obsproject/obs-studio#5010](https://github.com/obsproject/obs-studio/issues/5010).
- An Image source reloads an image file that is replaced within the same
  second. *All*, 2026-10-13. Issues
  [#547](https://github.com/obs-rust/obs-studio/issues/547),
  [#564](https://github.com/obs-rust/obs-studio/issues/564), PRs
  [#548](https://github.com/obs-rust/obs-studio/pull/548),
  [#565](https://github.com/obs-rust/obs-studio/pull/565), upstream
  [obsproject/obs-studio#10700](https://github.com/obsproject/obs-studio/issues/10700).
- A cropped Media Source with show/hide transitions plays again when it is
  shown after it played to its end. *All*, 2026-10-13. Issue
  [#572](https://github.com/obs-rust/obs-studio/issues/572), PR
  [#573](https://github.com/obs-rust/obs-studio/pull/573), upstream
  [obsproject/obs-studio#10852](https://github.com/obsproject/obs-studio/issues/10852).
- A looped Media Source plays an .mp3 or a file that does not start at 0
  again without a gap of silence and a held picture. *All*, 2026-10-13.
  Issue [#576](https://github.com/obs-rust/obs-studio/issues/576), PR
  [#577](https://github.com/obs-rust/obs-studio/pull/577), upstream
  [obsproject/obs-studio#12028](https://github.com/obsproject/obs-studio/issues/12028).
- An Image Slide Show set to an aspect ratio such as 16:9 gets a real size
  instead of 0x0. *All*, 2026-10-13. Issue
  [#578](https://github.com/obs-rust/obs-studio/issues/578), PR
  [#579](https://github.com/obs-rust/obs-studio/pull/579), upstream
  [obsproject/obs-studio#11696](https://github.com/obsproject/obs-studio/issues/11696).
- Changing Antialiasing on a Text (FreeType 2) source that reads from a file
  no longer blanks its characters. *All*, 2026-10-14. Issues
  [#484](https://github.com/obs-rust/obs-studio/issues/484),
  [#686](https://github.com/obs-rust/obs-studio/issues/686), PR
  [#711](https://github.com/obs-rust/obs-studio/pull/711).
- A local Media Source that closes when inactive plays again when it is
  restarted at its end, instead of showing nothing. *All*, 2026-10-14. PR
  [#732](https://github.com/obs-rust/obs-studio/pull/732).
- Changing the settings of a Media Source no longer can crash OBS while the
  old file is still opening. *All*, 2026-10-14. PR
  [#733](https://github.com/obs-rust/obs-studio/pull/733).
- Undo and Redo in the Filters dialog step through each change correctly
  when two settings change within half a second. *All*, 2026-10-14. Issue
  [#765](https://github.com/obs-rust/obs-studio/issues/765), PR
  [#766](https://github.com/obs-rust/obs-studio/pull/766), upstream
  [obsproject/obs-studio#10628](https://github.com/obsproject/obs-studio/issues/10628).
- Seeking a paused local Media Source shows the new frame at once, also when
  it caches the file. *All*, 2026-10-15. PR
  [#731](https://github.com/obs-rust/obs-studio/pull/731).
- An emptied legacy slideshow clears and reports its end also in manual,
  paused and deactivated modes, without a deadlock. *All*, 2026-10-15. PR
  [#735](https://github.com/obs-rust/obs-studio/pull/735).
- Next on the last slide of an Image Slide Show in "Once" mode ends the
  slideshow instead of going back to the first slide. *All*, 2026-10-15.
  Issue [#792](https://github.com/obs-rust/obs-studio/issues/792), PR
  [#793](https://github.com/obs-rust/obs-studio/pull/793).
- The Scroll filter shows sources wider or taller than 16384 pixels, such as
  long text tickers; before, they disappeared. *All*, 2026-10-15. Issue
  [#797](https://github.com/obs-rust/obs-studio/issues/797), PR
  [#798](https://github.com/obs-rust/obs-studio/pull/798), upstream
  [obsproject/obs-studio#10034](https://github.com/obsproject/obs-studio/issues/10034).
- Image Source and Image Slide Show follow the EXIF orientation with FFmpeg
  8.1 and later, so portrait photos are no longer sideways. *All*,
  2026-10-15. Issue
  [#821](https://github.com/obs-rust/obs-studio/issues/821), PR
  [#822](https://github.com/obs-rust/obs-studio/pull/822), upstream
  [obsproject/obs-studio#5223](https://github.com/obsproject/obs-studio/issues/5223).
- When you close a VST plug-in window with its own close button, the filter
  shows Open Plug-in Interface again and you can reopen the editor. *All*,
  2026-10-16. Issue
  [#1105](https://github.com/obs-rust/obs-studio/issues/1105), PR
  [#1106](https://github.com/obs-rust/obs-studio/pull/1106), upstream
  [obsproject/obs-studio#13580](https://github.com/obsproject/obs-studio/issues/13580).
- OBS no longer crashes when you pick a file, font or color in Properties
  after the source refreshed its properties while the dialog was open.
  *All*, 2026-10-16. Issue
  [#1112](https://github.com/obs-rust/obs-studio/issues/1112), PR
  [#1113](https://github.com/obs-rust/obs-studio/pull/1113), upstream
  [obsproject/obs-studio#10423](https://github.com/obsproject/obs-studio/issues/10423).
- A Media Source playing a file whose timestamps do not start at 0, such as
  MPEG-TS, shows the correct time and seeks to the correct position. *All*,
  2026-10-16. Issue
  [#1122](https://github.com/obs-rust/obs-studio/issues/1122), PR
  [#1123](https://github.com/obs-rust/obs-studio/pull/1123).
- A Media Source playing a WebM with alpha from a URL shows the video, not
  only the audio. *All*, 2026-10-16. Issue
  [#1131](https://github.com/obs-rust/obs-studio/issues/1131), PR
  [#1132](https://github.com/obs-rust/obs-studio/pull/1132).
- A Color Correction filter at its default settings leaves a high-precision
  (16-bit) source such as macOS Screen Capture as it is; before, it turned
  saturated colours (reds, greens) black or transparent. 8-bit sources look
  the same as before. *All*, 2026-10-19. Issue
  [#1471](https://github.com/obs-rust/obs-studio/issues/1471), PR
  [#1472](https://github.com/obs-rust/obs-studio/pull/1472), upstream
  [obsproject/obs-studio#10403](https://github.com/obsproject/obs-studio/issues/10403).
- An Image source given a picture larger than the GPU's texture size limit
  fails at once, with a log line that gives the size and the limit, instead
  of using gigabytes of memory and freezing OBS for seconds. *All*,
  2026-10-19. Issue
  [#1473](https://github.com/obs-rust/obs-studio/issues/1473), PR
  [#1474](https://github.com/obs-rust/obs-studio/pull/1474), upstream
  [obsproject/obs-studio#3427](https://github.com/obsproject/obs-studio/issues/3427).
- HDR Tone Mapping (and any filter that reads the colour space of what is
  below it) works whatever the filter order: an audio filter such as Gain
  or Noise Suppression below it no longer makes an HDR source look SDR.
  *All*, 2026-10-19. Issue
  [#1492](https://github.com/obs-rust/obs-studio/issues/1492), PR
  [#1493](https://github.com/obs-rust/obs-studio/pull/1493), port of
  [obsproject/obs-studio#13761](https://github.com/obsproject/obs-studio/pull/13761).
- Stopping a VLC Video Source playlist just as a track ends no longer crashes
  OBS, and no longer lets the next item start playing after the Stop.
  *All*, 2026-10-21. Issue
  [#1677](https://github.com/obs-rust/obs-studio/issues/1677), PR
  [#1678](https://github.com/obs-rust/obs-studio/pull/1678).
- Double-rate deinterlacing (Yadif 2x, Linear 2x, Discard 2x) shows both
  fields of the first frame when a source starts, restarts or comes back
  after a pause, instead of skipping the second field or holding the frame
  too long (an unbuffered source still moves straight on when its next
  frame is already waiting). *All*, pending merge. Issue
  [#1965](https://github.com/obs-rust/obs-studio/issues/1965), PR
  [#1995](https://github.com/obs-rust/obs-studio/pull/1995).
- Double-rate deinterlacing (Yadif 2x, Linear 2x, Discard 2x) shows every
  frame of a buffered source after it falls behind and catches up, instead
  of sometimes skipping the fields of a later frame and showing the frame
  after it a frame early. *All*, pending merge. Issue
  [#2009](https://github.com/obs-rust/obs-studio/issues/2009), PR
  [#2048](https://github.com/obs-rust/obs-studio/pull/2048).
- A camera, capture card or media source whose deinterlace mode is not one
  OBS knows (from an edited or generated scene collection, a plugin or a
  script) is drawn with deinterlacing off, instead of drawing nothing, and
  its Deinterlacing menu shows Disable. *All*, pending merge. Issue
  [#2090](https://github.com/obs-rust/obs-studio/issues/2090).

- Screen Capture (XSHM) keeps capturing the monitor it was set to when
  another monitor is unplugged or plugged in: it saves the RandR monitor's
  name, not only its place in the list, and shows no other monitor when its
  own goes away. *Linux*, 2026-10-25. Issue
  [#2261](https://github.com/obs-rust/obs-studio/issues/2261), upstream
  [obsproject/obs-studio#5502](https://github.com/obsproject/obs-studio/issues/5502).
- On macOS, a first browser source created by obs-websocket (`CreateInput`)
  renders. Before, CEF started on obs-websocket's thread, and no browser
  source rendered for the rest of the session, also ones added in the UI
  later. If that first source arrives while OBS quits, after Qt's main loop
  has stopped, OBS leaves CEF uninitialized and quits, instead of hanging.
  *macOS*, 2026-10-25 (the quit part pending merge). Issues
  [#2387](https://github.com/obs-rust/obs-studio/issues/2387),
  [#2391](https://github.com/obs-rust/obs-studio/issues/2391), PRs
  [#2390](https://github.com/obs-rust/obs-studio/pull/2390),
  [obs-rust/obs-browser#10](https://github.com/obs-rust/obs-browser/pull/10),
  [obs-rust/obs-browser#11](https://github.com/obs-rust/obs-browser/pull/11).

### Scenes and UI

- Edit > Undo is no longer clickable when there is nothing to undo; it was
  enabled again after loading a scene collection or closing the Filters
  dialog, even with an empty history. *All*, 2026-10-24. Issue
  [#2222](https://github.com/obs-rust/obs-studio/issues/2222).
- Theme metadata, theme string variables and effect annotation strings that
  contain escape characters no longer end with a stray closing quote. *All*,
  2026-10-09. Issue [#48](https://github.com/obs-rust/obs-studio/issues/48),
  PR [#54](https://github.com/obs-rust/obs-studio/pull/54).
- Hotkeys from saved files or plugins can no longer be bound to the left or
  right mouse button, so a normal click does not trigger a hotkey. *All*,
  2026-10-09. PR [#56](https://github.com/obs-rust/obs-studio/pull/56),
  upstream
  [obsproject/obs-studio#13403](https://github.com/obsproject/obs-studio/issues/13403).
- A source moved into or out of a hidden group now becomes active or
  inactive correctly, so it no longer plays audio while hidden or stays
  silent while visible. *All*, 2026-10-11. Issue
  [#241](https://github.com/obs-rust/obs-studio/issues/241), PR
  [#242](https://github.com/obs-rust/obs-studio/pull/242), upstream
  [obsproject/obs-studio#9961](https://github.com/obsproject/obs-studio/issues/9961).
- Moving one child of a group no longer moves the other children of the
  group across the canvas. *All*, 2026-10-11. Issue
  [#279](https://github.com/obs-rust/obs-studio/issues/279), PR
  [#280](https://github.com/obs-rust/obs-studio/pull/280), upstream
  [obsproject/obs-studio#9298](https://github.com/obsproject/obs-studio/issues/9298).
- Moving a scene into a group that the scene contains is refused instead of
  crashing OBS. *All*, 2026-10-12. Issue
  [#303](https://github.com/obs-rust/obs-studio/issues/303), PR
  [#304](https://github.com/obs-rust/obs-studio/pull/304), upstream
  [obsproject/obs-studio#10466](https://github.com/obsproject/obs-studio/issues/10466).
- Hiding a group now stops children that have show/hide transitions, so a
  hidden media source no longer plays. *All*, 2026-10-12. Issue
  [#324](https://github.com/obs-rust/obs-studio/issues/324), PR
  [#325](https://github.com/obs-rust/obs-studio/pull/325), upstream
  [obsproject/obs-studio#5551](https://github.com/obsproject/obs-studio/issues/5551).
- The Studio Mode Program view, a duplicated scene and an ungrouped item now
  draw scene items at the correct size. *All*, 2026-10-12. Issue
  [#345](https://github.com/obs-rust/obs-studio/issues/345), PR
  [#346](https://github.com/obs-rust/obs-studio/pull/346), upstream
  [obsproject/obs-studio#12171](https://github.com/obsproject/obs-studio/issues/12171).
- On X11 with a second keyboard layout, hotkeys on keys like [ and ] use the
  correct key instead of a number key. *Linux*, 2026-10-12. Issue
  [#349](https://github.com/obs-rust/obs-studio/issues/349), PR
  [#350](https://github.com/obs-rust/obs-studio/pull/350), upstream
  [obsproject/obs-studio#12941](https://github.com/obsproject/obs-studio/issues/12941).
- In Studio Mode, hiding a source that is only in the Preview now hides it,
  so "Shutdown source when not visible" works. *All*, 2026-10-12. Issue
  [#357](https://github.com/obs-rust/obs-studio/issues/357), PR
  [#358](https://github.com/obs-rust/obs-studio/pull/358), upstream
  [obsproject/obs-studio#4120](https://github.com/obsproject/obs-studio/issues/4120).
- Copy Transform and Paste Transform work with items inside groups and use
  canvas positions. *All*, 2026-10-12. Issue
  [#361](https://github.com/obs-rust/obs-studio/issues/361), PR
  [#362](https://github.com/obs-rust/obs-studio/pull/362), upstream
  [obsproject/obs-studio#5478](https://github.com/obsproject/obs-studio/issues/5478).
- Paste Filters pastes into every selected source, not only one. *All*,
  2026-10-12. Issue
  [#365](https://github.com/obs-rust/obs-studio/issues/365), PR
  [#366](https://github.com/obs-rust/obs-studio/pull/366), upstream
  [obsproject/obs-studio#4290](https://github.com/obsproject/obs-studio/issues/4290).
- The time in the media controls follows the media time, also on long files.
  *All*, 2026-10-12. Issue
  [#381](https://github.com/obs-rust/obs-studio/issues/381), PR
  [#382](https://github.com/obs-rust/obs-studio/pull/382), upstream
  [obsproject/obs-studio#11183](https://github.com/obsproject/obs-studio/issues/11183).
- A source added to an empty group with a bounding box keeps a valid
  transform and is drawn in the group's box. *All*, 2026-10-12. Issue
  [#387](https://github.com/obs-rust/obs-studio/issues/387), PR
  [#388](https://github.com/obs-rust/obs-studio/pull/388), upstream
  [obsproject/obs-studio#4245](https://github.com/obsproject/obs-studio/issues/4245).
- A fresh profile no longer logs a false "Failed to rename basic scene
  collection file" error. *All*, 2026-10-13. Issue
  [#155](https://github.com/obs-rust/obs-studio/issues/155), PR
  [#419](https://github.com/obs-rust/obs-studio/pull/419).
- The Decklink Captions and Decklink Output tools show their real menu and
  button text instead of raw text keys. *All*, 2026-10-13. Issue
  [#157](https://github.com/obs-rust/obs-studio/issues/157), PR
  [#421](https://github.com/obs-rust/obs-studio/pull/421).
- A normal shutdown no longer logs the warning "Tried to call
  obs_frontend_remove_event_callback with no callbacks!". *All*, 2026-10-13.
  Issue [#156](https://github.com/obs-rust/obs-studio/issues/156), PR
  [#425](https://github.com/obs-rust/obs-studio/pull/425).
- Turning Studio Mode off no longer sends scene-changed events for scenes
  that did not go live, so obs-websocket clients and Advanced Scene Switcher
  see no false CurrentProgramSceneChanged. *All*, 2026-10-13. PR
  [#481](https://github.com/obs-rust/obs-studio/pull/481), upstream
  [obsproject/obs-websocket#1150](https://github.com/obsproject/obs-websocket/issues/1150).
- Switching scenes quickly when a scene has a transition override no longer
  locks the program output on one scene. *All*, 2026-10-13. Issue
  [#555](https://github.com/obs-rust/obs-studio/issues/555), PR
  [#556](https://github.com/obs-rust/obs-studio/pull/556), upstream
  [obsproject/obs-studio#11914](https://github.com/obsproject/obs-studio/issues/11914).
- In Studio Mode, a preview scene set while a transition runs stays in the
  preview and is not replaced by the old program scene. *All*, 2026-10-13.
  Issue [#560](https://github.com/obs-rust/obs-studio/issues/560), PR
  [#561](https://github.com/obs-rust/obs-studio/pull/561), upstream
  [obsproject/obs-websocket#1320](https://github.com/obsproject/obs-websocket/issues/1320).
- On the Advanced Streaming tab, the Rescale Output resolution is greyed out
  when Rescale Output is Disabled. *All*, 2026-10-13. Issue
  [#585](https://github.com/obs-rust/obs-studio/issues/585), PR
  [#586](https://github.com/obs-rust/obs-studio/pull/586), upstream
  [obsproject/obs-studio#11908](https://github.com/obsproject/obs-studio/issues/11908).
- Profile and scene collection names that contain '&' show correctly in the
  menus. *All*, 2026-10-13. Issue
  [#587](https://github.com/obs-rust/obs-studio/issues/587), PR
  [#588](https://github.com/obs-rust/obs-studio/pull/588), upstream
  [obsproject/obs-studio#12853](https://github.com/obsproject/obs-studio/issues/12853).
- The selected built-in transition stays selected when you change the UI
  language. *All*, 2026-10-13. Issue
  [#590](https://github.com/obs-rust/obs-studio/issues/590), PR
  [#591](https://github.com/obs-rust/obs-studio/pull/591), upstream
  [obsproject/obs-studio#11844](https://github.com/obsproject/obs-studio/issues/11844).
- The Add Source dialog lists existing sources of a type by name again.
  *All*, 2026-10-13. Issue
  [#604](https://github.com/obs-rust/obs-studio/issues/604), PR
  [#605](https://github.com/obs-rust/obs-studio/pull/605), upstream
  [obsproject/obs-studio#13947](https://github.com/obsproject/obs-studio/issues/13947).
- In Settings > Stream, "Show all..." in the Service list no longer enables
  Apply or causes a false unsaved-changes prompt. *All*, 2026-10-13. Issue
  [#608](https://github.com/obs-rust/obs-studio/issues/608), PR
  [#609](https://github.com/obs-rust/obs-studio/pull/609), upstream
  [obsproject/obs-studio#9860](https://github.com/obsproject/obs-studio/issues/9860).
- Show Transition and Hide Transition from the Sources context menu apply to
  all selected sources, not only one. *All*, 2026-10-14. Issue
  [#757](https://github.com/obs-rust/obs-studio/issues/757), PR
  [#758](https://github.com/obs-rust/obs-studio/pull/758), upstream
  [obsproject/obs-studio#10448](https://github.com/obsproject/obs-studio/issues/10448).
- The Replay Buffer memory estimate in Settings > Output goes back to the
  normal colour when it is under the limit again. *All*, 2026-10-14. Issue
  [#759](https://github.com/obs-rust/obs-studio/issues/759), PR
  [#760](https://github.com/obs-rust/obs-studio/pull/760), upstream
  [obsproject/obs-studio#13171](https://github.com/obsproject/obs-studio/issues/13171).
- A Settings > Hotkeys field records Alt+C and Alt+O; before, these keys
  pressed Cancel or OK on KDE and GNOME. *Linux*, 2026-10-14. Issue
  [#763](https://github.com/obs-rust/obs-studio/issues/763), PR
  [#764](https://github.com/obs-rust/obs-studio/pull/764), upstream
  [obsproject/obs-studio#13385](https://github.com/obsproject/obs-studio/issues/13385).
- In Studio Mode with "Duplicate Sources", editing a source inside a group
  in Preview no longer changes Program before the transition. *All*,
  2026-10-15. Issue
  [#823](https://github.com/obs-rust/obs-studio/issues/823), PR
  [#824](https://github.com/obs-rust/obs-studio/pull/824), upstream
  [obsproject/obs-studio#4309](https://github.com/obsproject/obs-studio/issues/4309),
  [obsproject/obs-studio#6977](https://github.com/obsproject/obs-studio/issues/6977).
- Safe Mode and --only-bundled-plugins no longer show the Plugin Errors
  dialog, and removing a Missing plugin in the Plugin Manager removes the
  correct plugin. *All*, 2026-10-16. Issue
  [#1079](https://github.com/obs-rust/obs-studio/issues/1079), PR
  [#1080](https://github.com/obs-rust/obs-studio/pull/1080), upstream
  [obsproject/obs-studio#13948](https://github.com/obsproject/obs-studio/issues/13948),
  [obsproject/obs-studio#14005](https://github.com/obsproject/obs-studio/issues/14005).
- Shift+Tab out of a hotkey field in Settings > Hotkeys only moves the focus
  and no longer binds Shift to that action. *All*, 2026-10-16. Issue
  [#1114](https://github.com/obs-rust/obs-studio/issues/1114), PR
  [#1115](https://github.com/obs-rust/obs-studio/pull/1115), upstream
  [obsproject/obs-studio#12475](https://github.com/obsproject/obs-studio/issues/12475).
- In a tall Settings window, the audio track buttons on the Output page stay
  next to their Audio Track label. *All*, 2026-10-17. Issue
  [#1135](https://github.com/obs-rust/obs-studio/issues/1135), PR
  [#1136](https://github.com/obs-rust/obs-studio/pull/1136), upstream
  [obsproject/obs-studio#13286](https://github.com/obsproject/obs-studio/issues/13286).
- Screenshot (Source) of a 10-bit or 12-bit source saves the colors that OBS
  shows, not a much darker picture. *Linux, macOS*, 2026-10-17. Issue
  [#1143](https://github.com/obs-rust/obs-studio/issues/1143), PR
  [#1144](https://github.com/obs-rust/obs-studio/pull/1144), upstream
  [obsproject/obs-studio#13209](https://github.com/obsproject/obs-studio/issues/13209).
- Recording to a network or NAS path that stops responding no longer
  freezes the window every second: the free-space check while recording and
  in the Stats window runs off the UI thread. *All*, 2026-10-17. Issue
  [#1282](https://github.com/obs-rust/obs-studio/issues/1282), PR
  [#1336](https://github.com/obs-rust/obs-studio/pull/1336).
- Quitting OBS while recording no longer freezes the window while the
  recording finishes: OBS waits for the recording's stop signal with the
  event loop running (still up to 5 s, then a forced stop) instead of
  sleeping on the UI thread. *All*, 2026-10-17. Issue
  [#1283](https://github.com/obs-rust/obs-studio/issues/1283), PR
  [#1342](https://github.com/obs-rust/obs-studio/pull/1342).
- Recording shutdown waits until the output actually reports inactive,
  including the interval after its deactivate signal; a signal alone no
  longer permits premature shutdown. The UI keeps processing events within
  the existing timeout. *All*, 2026-10-18. Issue
  [#1391](https://github.com/obs-rust/obs-studio/issues/1391).
- Output deactivation publishes the inactive state before notifying signal
  consumers, preventing a shutdown observer from seeing an active output
  after deactivation. *All*, 2026-10-21. PR
  [#1643](https://github.com/obs-rust/obs-studio/pull/1643).
- Exit in the tray menu quits OBS when it was started with
  `--minimize-to-tray` and its window was never shown; before, nothing
  happened. *Linux (Wayland)*, 2026-10-18. Issue
  [#1400](https://github.com/obs-rust/obs-studio/issues/1400), upstream
  [obsproject/obs-studio#13494](https://github.com/obsproject/obs-studio/issues/13494).
- A source set to Darken blending leaves the background where it is
  transparent; before, it drew black there and cut a hole in the scene's
  alpha. *All*, 2026-10-18. Issue
  [#1404](https://github.com/obs-rust/obs-studio/issues/1404), upstream
  [obsproject/obs-studio#6591](https://github.com/obsproject/obs-studio/issues/6591).
- The Text (FreeType 2) source uses the font face that was picked when it
  is not the first face of its file: Bold and other styles of variable
  fonts, and the language faces of font collections such as Noto Sans CJK.
  Before, these rendered as the file's first face. *Linux*, 2026-10-18.
  Issue [#1409](https://github.com/obs-rust/obs-studio/issues/1409),
  upstream [obsproject/obs-studio#11819](https://github.com/obsproject/obs-studio/issues/11819).
- Help > Crash Reports > Upload Last Crash Report with no crash log to
  upload shows "No file to upload found or file was empty." Before, the
  dialog stayed at "Uploading..." for good. *Windows, macOS*, 2026-10-19.
  Issue [#1482](https://github.com/obs-rust/obs-studio/issues/1482), PR
  [#1483](https://github.com/obs-rust/obs-studio/pull/1483), upstream
  [obsproject/obs-studio#13401](https://github.com/obsproject/obs-studio/issues/13401),
  port of
  [obsproject/obs-studio#13414](https://github.com/obsproject/obs-studio/pull/13414).
- A plugin that reads or releases the Studio Mode T-Bar while Studio Mode is
  off (at startup, for example) no longer crashes OBS: the T-Bar reads as 0
  and a release does nothing. *All*, 2026-10-19. Issue
  [#1478](https://github.com/obs-rust/obs-studio/issues/1478), PR
  [#1479](https://github.com/obs-rust/obs-studio/pull/1479), port of
  [obsproject/obs-studio#13128](https://github.com/obsproject/obs-studio/pull/13128).
- Studio Mode with "Duplicate Scene" no longer logs "Attempted to add Scene
  without specifying a canvas!" on every transition, and a plugin's private
  copy of a scene on a second canvas (a vertical canvas, for example) is
  sized for that canvas instead of the main one. *All*, 2026-10-19. Issue
  [#1480](https://github.com/obs-rust/obs-studio/issues/1480), PR
  [#1481](https://github.com/obs-rust/obs-studio/pull/1481), port of
  [obsproject/obs-studio#12824](https://github.com/obsproject/obs-studio/pull/12824).
- Hotkey entries that bind nothing (no key and no modifier, saved as
  `{"key":"OBS_KEY_NONE"}`) are dropped when a profile or scene collection
  loads, so they are no longer saved back; such files still load as before.
  A load that clears a hotkey's bindings now tells Settings > Hotkeys and
  the Wayland global shortcuts. *All*, 2026-10-21. Issue
  [#1648](https://github.com/obs-rust/obs-studio/issues/1648), PR
  [#1649](https://github.com/obs-rust/obs-studio/pull/1649).
- On Linux, Settings > Hotkeys shows the arrow keys (Left, Right, Up, Down)
  in the chosen language; before, they were always in English. On every
  platform, a locale file or plugin with an empty numpad or mouse button
  name template no longer crashes OBS at startup; those keys keep their
  English names. *All*, 2026-10-21. Issue
  [#1656](https://github.com/obs-rust/obs-studio/issues/1656), PR
  [#1673](https://github.com/obs-rust/obs-studio/pull/1673).
- A scene collection with a negative scene item crop (edited by hand or
  written by another tool) loads with that crop at 0, as the Edit
  Transform dialog and obs-websocket set it; before, the item was drawn
  2 pixels wide or high. A crop past the integer range, the crop of a
  nested scene that grows far, and the size of a group whose sources are
  very far apart now get the nearest value in range instead of a wrong
  one. *All*, 2026-10-22. Issue
  [#1839](https://github.com/obs-rust/obs-studio/issues/1839), PR
  [#1840](https://github.com/obs-rust/obs-studio/pull/1840).
- A scene collection whose transition has an unknown scale type (edited by
  hand or written by another tool) loads with the default, which only
  scales sources down to fit; before, the transition drew its scenes with
  an undefined scale. *All*, pending merge. Issue
  [#2064](https://github.com/obs-rust/obs-studio/issues/2064), PR
  [#2080](https://github.com/obs-rust/obs-studio/pull/2080).

- Narrowing the main window and widening it again keeps the docks' relative
  sizes: docks side by side no longer come back all the same width after the
  window squeezed them to their minimum. *All*, 2026-10-24. Issue
  [#2225](https://github.com/obs-rust/obs-studio/issues/2225).

- Importing a profile from a folder path that ends in a slash (as the
  macOS folder dialog returns it) creates the profile in a folder of its
  own; before, its files were copied into the profiles folder itself and no
  profile showed up. *All*, pending merge. Issue
  [#2295](https://github.com/obs-rust/obs-studio/issues/2295), upstream
  [obsproject/obs-studio#13862](https://github.com/obsproject/obs-studio/issues/13862).

### Scripting

- A busy Lua source no longer makes the graphics thread poll its locks. OBS
  uses cached dimensions or skips that source's render call until the lock is
  available, allowing the rest of the frame to proceed. All; pending merge.
  Issue [#1275](https://github.com/obs-rust/obs-studio/issues/1275).

- quat_set, which Lua and Python scripts can call, now stores the x, y, z
  and w values in the correct order. *All*, 2026-10-10. PR
  [#86](https://github.com/obs-rust/obs-studio/pull/86).
- In Python scripts, sys.stdout and sys.stderr behave like normal text
  streams, so libraries that call isatty() and similar methods work. *All*,
  2026-10-11. Issue
  [#234](https://github.com/obs-rust/obs-studio/issues/234), PR
  [#235](https://github.com/obs-rust/obs-studio/pull/235), upstream
  [obsproject/obs-studio#4628](https://github.com/obsproject/obs-studio/issues/4628).
- A Lua script that makes a video source and enters graphics no longer
  freezes OBS; the source skips one frame instead. *All*, 2026-10-11. Issue
  [#238](https://github.com/obs-rust/obs-studio/issues/238), PR
  [#239](https://github.com/obs-rust/obs-studio/pull/239), upstream
  [obsproject/obs-studio#6674](https://github.com/obsproject/obs-studio/issues/6674).
- Python scripts can call obs_source_enum_filters to get a list of the
  filters on a source. *All*, 2026-10-11. Issue
  [#282](https://github.com/obs-rust/obs-studio/issues/282), PR
  [#283](https://github.com/obs-rust/obs-studio/pull/283), upstream
  [obsproject/obs-studio#12116](https://github.com/obsproject/obs-studio/issues/12116).
- Lua and Python scripts can read the obs-defs.h constants, such as
  OBS_ALIGN_*. *All*, 2026-10-12. Issue
  [#383](https://github.com/obs-rust/obs-studio/issues/383), PR
  [#384](https://github.com/obs-rust/obs-studio/pull/384), upstream
  [obsproject/obs-studio#8936](https://github.com/obsproject/obs-studio/issues/8936).
- A Lua or Python script that switches scenes from a timer no longer hangs
  OBS when it also has a frontend event callback; the scene changes right
  after the timer returns. *All*, 2026-10-18. Issue
  [#1416](https://github.com/obs-rust/obs-studio/issues/1416), PR
  [#1417](https://github.com/obs-rust/obs-studio/pull/1417), upstream
  [obsproject/obs-studio#3385](https://github.com/obsproject/obs-studio/issues/3385).

### Stability

- A scene item with a bounds type OBS doesn't know (a hand-edited or
  foreign scene collection, undo data, a plugin) no longer draws inside a
  phantom bounding box; it is loaded and set as no bounds. *All*, pending
  merge. Issue [#2281](https://github.com/obs-rust/obs-studio/issues/2281).
- Restoring a source's filters (undo, paste, scene collection load) no longer
  crashes when the source's size is read while a filter is being attached; a
  filter with no target yet reports a size of 0. *All*, 2026-10-24. Issue
  [#2172](https://github.com/obs-rust/obs-studio/issues/2172).
- DeckLink output no longer crashes when a saved output mode is one the
  device doesn't offer (settings from another card or firmware); the start
  fails with an error in the log. An HDR start with no video conversion
  starts as SDR instead of crashing. *All*, 2026-10-24. Issue
  [#2144](https://github.com/obs-rust/obs-studio/issues/2144).
- Upload Previous Log File no longer aborts OBS when no previous log exists;
  it reports that there is no log file. *Linux*, 2026-10-22. Issue
  [#1768](https://github.com/obs-rust/obs-studio/issues/1768), PR
  [#1824](https://github.com/obs-rust/obs-studio/pull/1824).

- OBS detects when writing a configuration file to disk fails, instead of
  treating the save as successful. *All*, 2026-10-10. PRs
  [#70](https://github.com/obs-rust/obs-studio/pull/70),
  [#71](https://github.com/obs-rust/obs-studio/pull/71),
  [#72](https://github.com/obs-rust/obs-studio/pull/72).
- Settings no longer crashes when no output supports the stream protocol.
  *All*, 2026-10-11. Issue
  [#217](https://github.com/obs-rust/obs-studio/issues/217), PR
  [#219](https://github.com/obs-rust/obs-studio/pull/219), upstream
  [obsproject/obs-studio#12843](https://github.com/obsproject/obs-studio/issues/12843).
- OBS on X11 no longer crashes in the hotkey thread when the X connection
  closes, for example at shutdown. *Linux*, 2026-10-11. Issue
  [#126](https://github.com/obs-rust/obs-studio/issues/126), PR
  [#231](https://github.com/obs-rust/obs-studio/pull/231).
- OBS no longer exits at startup on a Wayland desktop with more than one
  seat. *Linux*, 2026-10-11. Issue
  [#243](https://github.com/obs-rust/obs-studio/issues/243), PR
  [#244](https://github.com/obs-rust/obs-studio/pull/244), upstream
  [obsproject/obs-studio#12929](https://github.com/obsproject/obs-studio/issues/12929).
- OBS no longer crashes on text that the current locale cannot convert, such
  as a Latin-1 X11 window title in a UTF-8 locale. *Linux, Windows*,
  2026-10-11. Issue
  [#262](https://github.com/obs-rust/obs-studio/issues/262), PR
  [#263](https://github.com/obs-rust/obs-studio/pull/263).
- OBS also detects a crash or hang in the first session after installation
  and offers recovery on the next launch. *All*, 2026-10-13. Issue
  [#154](https://github.com/obs-rust/obs-studio/issues/154), PR
  [#424](https://github.com/obs-rust/obs-studio/pull/424).
- OBS no longer crashes when it quits after video initialization failed.
  *All*, 2026-10-13. Issue
  [#127](https://github.com/obs-rust/obs-studio/issues/127), PR
  [#432](https://github.com/obs-rust/obs-studio/pull/432).
- Changing the video resolution or profile no longer leaves raw outputs on
  freed memory, so obs-websocket GetOutputList and plugin outputs such as
  DistroAV no longer crash after it. *All*, 2026-10-13. Issue
  [#469](https://github.com/obs-rust/obs-studio/issues/469), PR
  [#470](https://github.com/obs-rust/obs-studio/pull/470), upstream
  [obsproject/obs-studio#1344](https://github.com/obsproject/obs-studio/issues/1344).
- Quitting OBS with a PipeWire camera source no longer prints thread
  warnings or crashes with a double free. *Linux*, 2026-10-13. Issue
  [#512](https://github.com/obs-rust/obs-studio/issues/512), PR
  [#513](https://github.com/obs-rust/obs-studio/pull/513).
- Two quick Start Streaming requests (obs-websocket StartStream or a deck
  button) no longer crash OBS. *All*, 2026-10-13. Issue
  [#538](https://github.com/obs-rust/obs-studio/issues/538), PR
  [#539](https://github.com/obs-rust/obs-studio/pull/539), upstream
  [obsproject/obs-studio#13283](https://github.com/obsproject/obs-studio/issues/13283).
- Starting OBS with --disable-missing-files-check no longer leaks memory on
  each scene collection load. *All*, 2026-10-13. Issue
  [#542](https://github.com/obs-rust/obs-studio/issues/542), PR
  [#543](https://github.com/obs-rust/obs-studio/pull/543), upstream
  [obsproject/obs-studio#11074](https://github.com/obsproject/obs-studio/issues/11074).
- Renaming or duplicating a fresh profile and then opening Settings no
  longer crashes OBS. *All*, 2026-10-13. Issue
  [#552](https://github.com/obs-rust/obs-studio/issues/552), PR
  [#553](https://github.com/obs-rust/obs-studio/pull/553), upstream
  [obsproject/obs-studio#12458](https://github.com/obsproject/obs-studio/issues/12458).
- OBS no longer can crash when the portal sleep inhibitor is destroyed while
  a reply is pending. *Linux*, 2026-10-13. Issue
  [#471](https://github.com/obs-rust/obs-studio/issues/471), PR
  [#656](https://github.com/obs-rust/obs-studio/pull/656).
- A script or plugin that removes scene items while it walks a scene no
  longer can make OBS use freed items. *All*, 2026-10-14. Issue
  [#490](https://github.com/obs-rust/obs-studio/issues/490), PR
  [#708](https://github.com/obs-rust/obs-studio/pull/708).
- On macOS, OBS no longer hangs when a window gets focus while the system
  looks up a generic app icon. *macOS*, 2026-10-14. Issues
  [#635](https://github.com/obs-rust/obs-studio/issues/635),
  [#725](https://github.com/obs-rust/obs-studio/issues/725), PR
  [#727](https://github.com/obs-rust/obs-studio/pull/727).
- A transition that is started towards a destination that already contains
  it is refused; before, OBS hung. *All*, 2026-10-14. PR
  [#737](https://github.com/obs-rust/obs-studio/pull/737).
- OBS starts when one core module cannot be loaded (for example obs-browser
  without CEF), logs a warning, and loads the other core modules. *Linux*,
  2026-10-14. Issues
  [#404](https://github.com/obs-rust/obs-studio/issues/404),
  [#749](https://github.com/obs-rust/obs-studio/issues/749), PR
  [#750](https://github.com/obs-rust/obs-studio/pull/750).
- OBS no longer crashes when a stream or recording starts, or obs-websocket
  lists outputs, after a resolution change. *All*, 2026-10-16. Issue
  [#1108](https://github.com/obs-rust/obs-studio/issues/1108), PR
  [#1109](https://github.com/obs-rust/obs-studio/pull/1109), upstream
  [obsproject/obs-studio#12213](https://github.com/obsproject/obs-studio/issues/12213).

- Browser panels no longer crash during cold startup with CEF 150, and
  closing OBS while its browser engine starts no longer hangs. *Linux*,
  2026-10-17. PR [#1333](https://github.com/obs-rust/obs-studio/pull/1333),
  dependency [obs-browser#1](https://github.com/obs-rust/obs-browser/pull/1).
- SIGTERM while recording or streaming quits OBS and finishes the
  recording, even with "Show confirmation on exit while outputs are
  active" on, and SIGTERM while a dialog is open (also during startup)
  quits too; before, OBS waited until the sender killed it and the
  recording was cut off. *Linux, macOS*, 2026-10-18. Issue
  [#1402](https://github.com/obs-rust/obs-studio/issues/1402), upstream
  [obsproject/obs-studio#10011](https://github.com/obsproject/obs-studio/issues/10011).
- A plugin or script that resets the video to an output width of 2 or 3
  gets an invalid-parameter error and keeps the current video; before, the
  reset failed with no video left, and OBS could then crash. *All*,
  2026-10-22. Issue
  [#1713](https://github.com/obs-rust/obs-studio/issues/1713), PR
  [#1734](https://github.com/obs-rust/obs-studio/pull/1734).
- A plugin or script that renames a source to no name (NULL) no longer
  crashes OBS; the source gets an unnamed name, as with an empty name. A
  rename to a name that is taken now logs the requested name as the
  duplicate, not the source's old name. *All*, 2026-10-22. Issue
  [#1787](https://github.com/obs-rust/obs-studio/issues/1787), PR
  [#1804](https://github.com/obs-rust/obs-studio/pull/1804).
- A plugin or script that asks whether a NULL output protocol is registered,
  or lists the outputs for one, no longer crashes OBS; no output supports a
  protocol that is not there. The auto-configuration wizard's bandwidth test
  reports that no output fits, instead of crashing, when its stream service
  could not be created (for example without the rtmp-services plugin).
  *All*, 2026-10-22. Issue
  [#1827](https://github.com/obs-rust/obs-studio/issues/1827), PR
  [#1828](https://github.com/obs-rust/obs-studio/pull/1828).
- A tool or plugin host that adds module search paths with
  obs_add_module_path() gets a defined module type for the modules found
  there (undefined: neither core nor legacy), so the disabled list applies
  to them; before, the type was whatever was in memory. *All*, 2026-10-23.
  Issue [#1900](https://github.com/obs-rust/obs-studio/issues/1900), PR
  [#1906](https://github.com/obs-rust/obs-studio/pull/1906).
- A source type that a plugin fails to register (libobs rejects it, for
  example a composite audio source or one without a name) is no longer
  counted as that plugin's: its sources show as missing in the Sources list
  instead of as provided by a loaded plugin, and the type is not credited to
  the plugin if something else registers it later. A plugin or script that
  adds a type id to no module (NULL) no longer leaks a copy of it. *All*,
  pending merge. Issue [#1953](https://github.com/obs-rust/obs-studio/issues/1953), PR
  [#1960](https://github.com/obs-rust/obs-studio/pull/1960).
- A plugin or script that gives an output an audio mixer index OBS does not
  have (6 or more) no longer changes the output's mixers; the call is
  ignored with a warning. A raw output with a preferred (scaled) size but no
  video no longer crashes OBS when asked for its video conversion; it has
  none. *All*, pending merge. Issue
  [#2127](https://github.com/obs-rust/obs-studio/issues/2127).
- A canvas that a plugin or script creates, or that a scene collection
  (edited by hand or written by another tool) loads, keeps only the public
  canvas flags. Before, a removed bit in its flags made the canvas count as
  removed from the start, so removing it later did not tell OBS or other
  plugins, which kept it; unknown bits were saved back. A canvas that fails
  to set up is no longer leaked. *All*, pending merge. Issue
  [#2141](https://github.com/obs-rust/obs-studio/issues/2141).
- When the system cannot set up a lock or start a thread, OBS reports it
  instead of carrying on as if it had worked: a source, output or other
  object whose lock fails is not created, and a source that fails to set up
  no longer crashes OBS while it is cleaned up; an output whose reconnect
  thread does not start no longer stays "reconnecting" for good; and the
  screen saver inhibitor reports that it could not start. *All*, pending
  merge. Issue [#2152](https://github.com/obs-rust/obs-studio/issues/2152).
- A filter that is removed (in the Filters dialog, by a script or by
  obs-websocket) while OBS is drawing it no longer crashes OBS when it has
  nothing to draw (for example while it is disabled or has a size of 0); it
  draws nothing for that frame. *All*, pending merge. Issue
  [#2160](https://github.com/obs-rust/obs-studio/issues/2160).
- When the system cannot set up the event that OBS waits on, OBS no
  longer crashes on it: a task that OBS hands to another of its threads
  still runs once and is waited for; recording through the buffered file
  writer or ffmpeg-mux, macOS window capture and the screen saver
  inhibitor fail to start instead; macOS display and screen capture do not
  start their stream; and a streaming service's ingest list refresh is not
  waited for. macOS display and screen capture also no longer free the
  same event twice after an update that could not restart them. *All*,
  pending merge. Issue [#2164](https://github.com/obs-rust/obs-studio/issues/2164).
- A plugin that registers a source, output, encoder or service with a
  structure size that ends partway through its type data fields no longer
  crashes OBS when the registration is refused; the type data is freed only
  when the size holds both fields whole. *All*, pending merge. Issue
  [#2195](https://github.com/obs-rust/obs-studio/issues/2195).
- A plugin that registers a source, output, encoder or service with a
  structure size that ends partway through a callback or its type data no
  longer leaves that field partly copied, which crashed OBS when it was
  called later or at shutdown; a field is kept only when the size holds it
  whole. *All*, pending merge. Issue
  [#2203](https://github.com/obs-rust/obs-studio/issues/2203).
- A plugin, script or headless tool that creates an output before OBS has
  set up video no longer crashes OBS; the output has no video until one is
  set, and it cannot start without it. *All*, pending merge. Issue
  [#2237](https://github.com/obs-rust/obs-studio/issues/2237).
- A plugin, script or headless tool that asks for the main texture, adds or
  removes a raw video callback, asks whether NV12 or P010 texture output is
  active, or reads the position or scale of a scene item that is no longer
  in a scene before OBS has set up video no longer crashes OBS; it gets no
  texture, a refused callback with a warning, "not active" and a 0x0
  canvas. *All*, pending merge. Issue
  [#2253](https://github.com/obs-rust/obs-studio/issues/2253).
- An SDR white level or HDR nominal peak level of 0, below 0, NaN or
  infinite (from an edited profile, a plugin or a script) no longer makes
  HDR and scRGB sources draw solid white, black or garbage: the level is
  ignored with a warning in the log and the previous one stays. With
  graphics up, the levels also start at 300 and 1000 nits instead of 0
  until something sets them. *All*, pending merge. Issue
  [#2209](https://github.com/obs-rust/obs-studio/issues/2209).
- A plugin or script that resets an effect parameter to its default with
  a parameter it failed to look up no longer crashes OBS; the call logs an
  error and does nothing, like the other effect parameter setters. *All*,
  pending merge. Issue
  [#2210](https://github.com/obs-rust/obs-studio/issues/2210).
- Asking whether a video encoder uses NV12 or P010 texture input no longer
  crashes OBS when the encoder has no video yet, or when its video belonged
  to a canvas that has been removed; the answer is "not active", so an
  encoder plugin falls back to raw frames. *All*, pending merge. Issue
  [#2267](https://github.com/obs-rust/obs-studio/issues/2267).

### Security hardening

- Malformed data from an RTMP server (a zero or negative chunk size, deep or
  broken AMF data, oversized fields) can no longer hang the stream, crash
  OBS or corrupt memory. *All*, 2026-10-11. Issues
  [#100](https://github.com/obs-rust/obs-studio/issues/100),
  [#102](https://github.com/obs-rust/obs-studio/issues/102),
  [#173](https://github.com/obs-rust/obs-studio/issues/173),
  [#209](https://github.com/obs-rust/obs-studio/issues/209), PRs
  [#115](https://github.com/obs-rust/obs-studio/pull/115),
  [#116](https://github.com/obs-rust/obs-studio/pull/116),
  [#188](https://github.com/obs-rust/obs-studio/pull/188),
  [#228](https://github.com/obs-rust/obs-studio/pull/228),
  [#230](https://github.com/obs-rust/obs-studio/pull/230),
  [#232](https://github.com/obs-rust/obs-studio/pull/232).
- obs-websocket closes a connection that sends deeply nested JSON or
  MessagePack before authentication, and OBS no longer crashes from it.
  *All*, 2026-10-13. Issue
  [#99](https://github.com/obs-rust/obs-studio/issues/99), PRs
  [#477](https://github.com/obs-rust/obs-studio/pull/477),
  [#497](https://github.com/obs-rust/obs-studio/pull/497),
  [#502](https://github.com/obs-rust/obs-studio/pull/502),
  [#503](https://github.com/obs-rust/obs-studio/pull/503).

- Rust graphics preserves separate C trigonometric calls on macOS and signaling
  NaN rounding on macOS/Windows. *macOS, Windows*, 2026-10-18.
  PR [#1398](https://github.com/obs-rust/obs-studio/pull/1398).
- Downloads and remote text requests no longer ask servers for brotli: what
  comes back can no longer reach a Brotli decoder with a public
  out-of-bounds write while the packaged libraries still carry it. *All*,
  2026-10-24. Issue
  [#2211](https://github.com/obs-rust/obs-studio/issues/2211), PR
  [#2213](https://github.com/obs-rust/obs-studio/pull/2213).

### Building from source

- Building with `-DENABLE_CORE_MODULES=OFF` works again: libobs no longer
  fails to compile a core module list that was never generated. *All*,
  pending merge. Issue
  [#2280](https://github.com/obs-rust/obs-studio/issues/2280);
  obsproject/obs-studio#14013.

## Development builds

- **Cargo application command.** `./native-build` builds and stages a Linux hybrid
  OBS development application with selectable profiles and plugin features,
  including default NVENC support and its capability helper.
  *Linux*, 2026-10-22. Issue [#1332](https://github.com/obs-rust/obs-studio/issues/1332),
  PRs [#1725](https://github.com/obs-rust/obs-studio/pull/1725),
  [#1772](https://github.com/obs-rust/obs-studio/pull/1772).
- **Explicit Qt SDKs for Cargo builds.** `OBS_QT_TARGET_PREFIX` and
  `OBS_QT_HOST_PREFIX` select the target Qt SDK and its host tools. Cargo resolves
  Qt Core, Gui and Widgets from the SDK's exported targets, validates the
  architecture, Qt version and CRT, and cross builds no longer fall back to
  host pkg-config. *Linux; Windows and macOS SDK resolution*, 2026-10-22.
  Issue [#1869](https://github.com/obs-rust/obs-studio/issues/1869).
