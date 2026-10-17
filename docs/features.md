# Features ledger

Everything OBS-Studio-Rust does differently from upstream OBS Studio, for a
user: new features, changed behavior, and fixed upstream bugs. A pull
request announces a change once; this file keeps the list (#1250).

The backfill of the pull requests merged before this file existed is in
progress (#1250). Until it is done, this list is not complete.

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
  with the upstream issue (obsproject/obs-studio#N) when there is one.

## Features

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
- **Refused output starts are logged.** A start of streaming, recording,
  the replay buffer or the virtual camera that is refused while outputs are
  disabled writes a log line, where before obs-websocket replied "success"
  and nothing happened. *All*, 2026-10-15. Issue
  [#817](https://github.com/obs-rust/obs-studio/issues/817), PR
  [#818](https://github.com/obs-rust/obs-studio/pull/818).
- **obs-websocket changes are saved.** Requests that change the scene
  collection (volume, mute, input settings, transforms and others) save it,
  so a crash or a hard stop no longer loses them. *All*, 2026-10-16. Issue
  [#811](https://github.com/obs-rust/obs-studio/issues/811), PR
  [#812](https://github.com/obs-rust/obs-studio/pull/812).

### GPU status

- **GPU acceleration light.** A light in the status bar shows whether OBS
  renders on a hardware GPU (green), in software on a machine without a GPU
  (yellow), or in software although a GPU is present, after a graphics
  failure or over the frame budget (red). Clicking it explains the state.
  The startup log and `GetStats.gpuAcceleration` in obs-websocket have the
  same state ([gpu-status.md](gpu-status.md)). *All*, 2026-10-15. Issue
  [#846](https://github.com/obs-rust/obs-studio/issues/846), PRs
  [#890](https://github.com/obs-rust/obs-studio/pull/890),
  [#894](https://github.com/obs-rust/obs-studio/pull/894),
  [#929](https://github.com/obs-rust/obs-studio/pull/929),
  [#1006](https://github.com/obs-rust/obs-studio/pull/1006),
  [#1010](https://github.com/obs-rust/obs-studio/pull/1010),
  [#1178](https://github.com/obs-rust/obs-studio/pull/1178).
- **`GPU: x%` label.** The light shows the render GPU's utilization next to
  the CPU usage, or `CPU rendering` in software; obs-websocket gets the same
  reading. Sources: NVML for NVIDIA, `gpu_busy_percent` for AMD. *Linux*,
  2026-10-17. Issue
  [#1025](https://github.com/obs-rust/obs-studio/issues/1025), PRs
  [#1222](https://github.com/obs-rust/obs-studio/pull/1222),
  [#1233](https://github.com/obs-rust/obs-studio/pull/1233),
  [#1236](https://github.com/obs-rust/obs-studio/pull/1236).

### Audio

- **ALSA captures 24-bit devices at 24 bits.** A USB audio interface that
  offers `S24_3LE` and `S16_LE` is opened at 24 bits, where before OBS
  tried 16 bits first. *Linux*, 2026-10-16. Issue
  [#1103](https://github.com/obs-rust/obs-studio/issues/1103), PR
  [#1104](https://github.com/obs-rust/obs-studio/pull/1104).

### Performance

- **Faster launch on NVIDIA machines.** 4.3 times faster to the main
  window and 19 % less memory on an NVIDIA streaming machine (see the
  README): the VAAPI probe skips decode-only drivers, the NVENC test result
  is reused while the driver does not change, the AppImage loads only
  NVIDIA's EGL vendor, and the GPU status probe runs on a thread. *Linux*,
  2026-10-17. Issues
  [#1168](https://github.com/obs-rust/obs-studio/issues/1168),
  [#1170](https://github.com/obs-rust/obs-studio/issues/1170),
  [#1171](https://github.com/obs-rust/obs-studio/issues/1171),
  [#1172](https://github.com/obs-rust/obs-studio/issues/1172), PRs
  [#1175](https://github.com/obs-rust/obs-studio/pull/1175),
  [#1176](https://github.com/obs-rust/obs-studio/pull/1176),
  [#1177](https://github.com/obs-rust/obs-studio/pull/1177),
  [#1178](https://github.com/obs-rust/obs-studio/pull/1178).

### Platforms and packages

- **Builds with Qt 6.8.** The frontend builds against distribution Qt 6.8,
  for example on Debian 13. *Linux*, 2026-10-16. Issue
  [#1015](https://github.com/obs-rust/obs-studio/issues/1015), PR
  [#1017](https://github.com/obs-rust/obs-studio/pull/1017).

## Fixed upstream bugs

Bugs inherited from OBS Studio that are fixed here. The backfill (#1250)
adds them.
