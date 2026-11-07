# Native graph batch 2: libobs g–o slice 5

Refs #1390. Continue from `test/1390-b2-libobs-go4` at
`f3d4291f69aeb92dc9b985fbc20546c9c8bf8813`; no rollup PR or hosted CI.

Register two unchanged C executables (15 cases) with real `OBS::libobs`:

| Test | Cases | Original contract |
| --- | --- | --- |
| `test_image_file` | 11 | All hosts, fake texture device, repository data directory; no GPU/display |
| `test_obs_outputs_net_if_host` | 4 | `NOT OS_WINDOWS`, original root/POSIX include directories, IPv4/IPv6 and local interface enumeration |

Both retain direct execution, original library order, no timeout, no resource
lock and no skip override. The image fixture includes pthread types through
libobs, so the graph declares its Windows `OBS::w32-pthreads` interface.
No platform fact named `UNIX` or pkg-config simde lookup is introduced.
Registration checks live in the existing Application category; no new
repository Rust test executable. Existing C tests/public headers, WebSocket
and workflow files are unchanged.

## Evidence and limits

Image registration RED `2a4fc3be2` fails with missing target (left `Null`);
GREEN `7a5ff3071`. Network-interface RED `cafed86f1` likewise fails for the
missing target, followed by its GREEN graph registration. Both final checks
pass (`local-test-ojYZe3.log`).

GCC and Clang each exercise the actual Cargo graph, canonical CMake consumer
and preserved original CMake fragment. The network-interface executable builds
and passes all four cases in every consumer/compiler combination; `ldd` in the
Cargo replay confirms the installed OBS 32.1.2 core.

The image executable compiles but fails to link against that older installed
core in all three consumers with both compilers: it lacks the current
`gs_image_file_ex_*` exports. No image runtime success is claimed, no stubs are
added, and no failure is bypassed. The replay orchestrator returns zero only
for the asserted successful network-interface runs; all six image build
failures remain explicit in `/tmp/b2-libobs-go5/results.json`.
Replay report: `local-test-DdXQsX.log`.

Original and canonical CMake inventories agree for Linux, Windows, macOS,
FreeBSD and OpenBSD: image always present, network-interface absent on Windows,
no timeout/skip/resource changes (`local-test-okr0da.log`). These are selection
and configure simulations, not native Windows/macOS compilation or runtime.
No simde pkg-config package is required by the fixture.

Focused graph-slice, sorted-list, sanitizer-harness and nextest-budget guards
pass (`local-test-tR097H.log`). Scoped Application/obs-build Clippy and the
whole-graph legacy replacement check are recorded in the worker response.
Rustfmt and `git diff --check` pass.

Test cost: both registration checks over 25 reduced debug runs, longest 0.040594 s, peak child RSS 11576 KiB; no test-owned threads.

Required rollup validation remains current-head hybrid and C-reference
runtime, exact exports and native Windows/macOS builds. This slice proves no
application/profile/protocol/hardware completion. Full workspace lint and
unittest were not run; no target pruning or soldr GC.

## Remaining candidates

43 inherited literal declarations remain deferred; this is not an evaluated
CTest count. The following inherited classifications scope follow-up work,
not a claim that none can ever run headlessly:

- test_gl_dynamic_texture: deferred: separate graphics/host/plugin integration; bounded five-test batch.
- test_graphics_ffmpeg_host: deferred: FFmpeg/platform/Rust-static composition.
- test_graphics_ffmpeg_pq_steps_rust: deferred: FFmpeg/platform/Rust-static composition.
- test_graphics_ffmpeg_route_steps_rust: deferred: FFmpeg/platform/Rust-static composition.
- test_graphics_ffmpeg_wic_checks_rust: deferred: FFmpeg/platform/Rust-static composition.
- test_graphics_immediate_steps_rust: deferred: FFmpeg/platform/Rust-static composition.
- test_graphics_state_stacks_rust: deferred: FFmpeg/platform/Rust-static composition.
- test_hotkey_background_ui: display/X11 interaction.
- test_hotkey_background_use_key: display/X11 interaction.
- test_hotkey_press_state: display/X11 interaction.
- test_image_size_precheck: graphics module dependency, outside headless no-graphics assignment.
- test_inhibit_portal: deferred: host session/DBus integration.
- test_inhibit_sleep: deferred: host session/DBus integration.
- test_inhibit_sleep_dbus: deferred: host session/DBus integration.
- test_jack_input: deferred: separate graphics/host/plugin integration; bounded five-test batch.
- test_lua_source_graphics_deadlock: deferred: separate graphics/host/plugin integration; bounded five-test batch.
- test_media_alpha_network: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_cached_paused_seek: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_cover_art_monitor: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_cover_art_seek: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_format_change: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_gif_last_frame: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_loop_start_time: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_network_timeout: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_packet_eof_leak: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_play_after_stop: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_restart_first_frame: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_restart_stale_frame: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_seek_keyframe: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_source_options_lifetime: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_status_threading: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_sync_offset_restart: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_media_ts_start_time: deferred: media-playback/FFmpeg/runtime fixture composition.
- test_nix_startup_log: deferred: startup/display or platform characterization.
- test_nix_wayland_keys_steps: deferred: startup/display or platform characterization.
- test_nsgif_decode_rust: Rust-static linkage; static-worker class.
- test_obs_outputs_rtmp_host: deferred: host/TLS dependency composition.
- test_obs_startup_failure_diagnostics: deferred: X11 interposition.
- test_opus_mono_bitrate: deferred: separate graphics/host/plugin integration; bounded five-test batch.
- test_output_end_capture_thread_join: deferred: graphics/mux or Qt integration.
- test_output_filename_reserve: deferred: graphics/mux or Qt integration.
- test_output_stop_without_packets: deferred: graphics/mux or Qt integration.
- test_output_timer: deferred: graphics/mux or Qt integration.
