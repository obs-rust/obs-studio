# Batch 5 built-libobs replay — Refs #1390

This supersedes the installed-core runtime descriptions in [b5-gl.md](b5-gl.md)
and [b5-gl2.md](b5-gl2.md). Their original RED/GREEN and compiler evidence
remains historical evidence, not current-core parity evidence.

Batch 5 was rebased from `c8f429ec2` onto batch 4 `b0c8a2af9` with exit 0.
Range-diff preserves all four original commits unchanged. The runtime now calls
`built_core()` with `OBS_GRAPH_LIBOBS=hybrid|c-reference` and builds its OpenGL
renderer against that exact artifact. Both graph consumers import the same core
and renderer; neither uses an installed libobs. Effects are staged under
`share/obs/libobs`, matching the built core's generated `OBS_DATA_PATH`.
The CMake replay uses `RelWithDebInfo`, as Tier 2 does.
Its configuration working directory has its own matching effect-data tree.

The rebased runtime fails `graph_runtimes_never_link_an_installed_libobs` with
exit 101, identifying all three installed-core references. The converted runtime
passes that guard. RED `ade3f89f8` expands the existing registration contract and
fails with exit 101 because `test_preview_only_hide` is absent. The graph adds
four unchanged executables: preview-only hide (6 cases), raw-output video reset
(4), transition audio clock (1), and removed-child audio (1).
Together with visibility (8) and XComposite error replies (2), the slice has
six native executables and 22 native cases. The canonical graph has 525 target
and test records; this is not an evaluated all-host total.

Sources, linkage, commands, effect dependencies, Linux/renderer guards and
60/120-second timeouts are preserved. Preview-only hide retains `OBS::caption`;
audio rescale is excluded after its hybrid failure. Each legacy fragment points to the graph rather than
registering the executable twice. Existing C tests, public headers, WebSocket
and workflows are unchanged. No new Rust test executable is introduced.

The existing registration contract evaluates Linux, Windows and macOS with the
renderer both enabled and disabled. The real CMake replay additionally checks
Windows/macOS, renderer-off and capture-owner-off selections; owner-off retains
the five non-capture tests. Missing requirement names remain errors.

The expanded registration contract passes 25 reduced-debug runs: longest
0.098 seconds, peak child RSS 14,792 KiB, no own Rust threads. The real native
compilation/replay retains the existing native-build budget exception.

Full `./lint` exits 1 solely because Soldr refuses to build the missing
nightly-2026-05-28 Dylint driver under clud. Formatting, Python self-tests,
workspace Clippy and all five feature-set Clippy lanes pass. The expanded
contract's initial complexity error was corrected by extracting its library
expectations, with no assertion changes. Dylint remains unverified; no toolchain
or dependency workaround was applied. Focused final registration and
installed-core guard checks both pass.

Final GCC/Xvfb runtime validation passes against both built cores: hybrid
142.62 seconds, C-reference 148.24 seconds, exit 0 for each. Every mode executes
all six native tests through Cargo and again through CMake/CTest (22 native
cases per consumer), rejects skipped output on both streams, and passes the
Windows/macOS, no-renderer and no-owner inventory checks. No scout hook remains
in committed source. Rust compilation in the container also goes through soldr;
the resulting test executable runs directly under Xvfb with the image's GCC.

The scout uses the original CMake fragments, GCC, private Xvfb and the Tier 2
image `obs-rust-linux-validate:cb8f699f47ef269d`. Per-test logs are retained in
`build_ci_audit/b5-rebase-built/`. C-reference fully passes eleven of the fourteen
previously rejected fixtures, with no hidden skips. Replay-buffer naming still
reports CTest Passed despite missing its required `encoded_packet` callback and
failing output creation; it is rejected. Replay-buffer pause/stop fails
`stop_while_recording_paused_completes`; XComposite window events fails its
elapsed-time bound. Neither failure was weakened or registered here. Hybrid also rejects audio
rescale: `fade_with_rescaled_encoder` reports no program audio during the fade.
The RED contract initially included that candidate; the final accepted set
excludes it under the no-failed-fixture rule.

| Previously rejected fixture | Hybrid | C-reference | Added here |
|---|---|---|---|
| `test_preview_only_hide` | Pass | Pass | Yes |
| `test_raw_output_video_reset` | Pass | Pass | Yes |
| `test_recording_shutdown` | Pass | Pass | No |
| `test_replay_buffer_naming` | Invalid setup | Invalid setup | No |
| `test_replay_buffer_pause_stop` | Pass | Fail | No |
| `test_transform_clipboard` | Pass | Pass | No |
| `test_transition_audio_clock` | Pass | Pass | Yes |
| `test_transition_audio_rescale` | Fail | Pass | No |
| `test_transition_removed_child_audio` | Pass | Pass | Yes |
| `test_xcomposite_window_events` | Pass | Fail | No |
| `test_xshm_monitor_hotplug` | Pass | Pass | No |
| `test_xshm_monitor_late` | Pass | Pass | No |
| `test_xshm_monitor_move` | Pass | Pass | No |
| `test_xshm_resolution_change` | Pass | Pass | No |

Pass means every native case ran, with no skipped cases or rejected fixture
objects. The C-reference scout used all fourteen original CMake fragments.
For hybrid, the four migrated fixtures run through the graph, and the other ten
use the scout or, for rejected audio rescale, the graph run that exposed its
failure. Empty CTest inventories left by the migrated fragments are not counted.

This slice does not complete #1390. Full application/profile/protocol validation,
dynamic export comparison, hardware fixtures, native Windows/macOS execution and
full workspace tests remain separate acceptance work. Final Clang replay was
not run; the earlier installed-core Clang evidence remains historical only.
