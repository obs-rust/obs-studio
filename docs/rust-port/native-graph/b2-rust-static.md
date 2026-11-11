# Batch 2: Rust-static source steps (#1390)

Based on #3832. Register eight existing C characterization executables in
`test/cmocka/build/test-graph.d/`, with a separate registration module in the
existing Application category binary. The C test sources and public headers
remain unchanged. These tests require no display, hardware or OBS startup.

## Preserved contracts

- `test_source_audio_input_steps_rust`
- `test_source_audio_render_steps_rust`
- `test_source_audio_timing_steps_rust`
- `test_source_ptm_gate_steps_rust`
- `test_source_size_route_steps_rust`
- `test_source_volume_apply_steps_rust`
- `test_source_frame_copy_steps_rust`
- `test_transition_render_steps_rust`

Every target/test requires `HAVE_LIBOBS_RUST_STATIC`, has exactly its original
single C test source, the `libobs` include directory, and ordered linkage to
`libobs_rust-static` then cmocka. Audio render, size route, volume apply, frame
copy and transition render retain `m` only when not MSVC. No wrapper, sanitizer,
compile definition, runtime skip code or timeout is added. The original C
reference entries and their sanitizers remain intact. Explicit watched headers
track the archive-only tests' internal declarations. None includes threading.h,
obs.h, obsconfig.h or SIMDe, so no pthread/header dependency is introduced.

The module asserts registration, exact source/link order and empty added policy,
and selects archive absent/present for Linux, macOS and Windows. Its opt-in
Linux runtime contract imports the real production Rust archive, compiles and
runs all eight unchanged suites with the Cargo native graph, then configures,
builds and runs the same selected graph through the real CMake consumer. It
also verifies CTest inventory with the archive disabled and re-enabled and
preserves absent TIMEOUT properties. No stub archive or libobs substitute is used.

## Scope left for later batches

`test_async_frame_pacing_steps_rust` and `test_async_conversion_plan_rust` were
already in #3832 and are excluded. The following original archive-linked tests
are deferred to keep this batch focused on eight archive-only source suites;
this is not a claim that their dependencies cannot be supported:

- `test_async_upload_steps_rust` — outside this eight-suite batch.
- `test_audio_monitor_delay_steps_rust` — outside this eight-suite batch.
- `test_coreaudio_device_rules_rust` — outside this eight-suite batch.
- `test_coreaudio_pacing_steps_rust` — outside this eight-suite batch.
- `test_deinterlace_draw_plan_steps_rust` — outside this eight-suite batch.
- `test_deinterlace_pacing_steps_rust` — outside this eight-suite batch.
- `test_deinterlace_select_steps_rust` — outside this eight-suite batch.
- `test_effect_param_types_steps_rust` — outside this eight-suite batch.
- `test_effect_parser_blocks_rust` — outside this eight-suite batch.
- `test_effect_parser_lifecycle_rust` — outside this eight-suite batch.
- `test_effect_parser_params_rust` — outside this eight-suite batch.
- `test_effect_parser_passes_rust` — outside this eight-suite batch.
- `test_effect_parser_shader_call_rust` — outside this eight-suite batch.
- `test_graphics_ffmpeg_pq_steps_rust` — outside this eight-suite batch.
- `test_graphics_ffmpeg_route_steps_rust` — outside this eight-suite batch.
- `test_graphics_ffmpeg_wic_checks_rust` — outside this eight-suite batch.
- `test_graphics_immediate_steps_rust` — outside this eight-suite batch.
- `test_graphics_state_stacks_rust` — outside this eight-suite batch.
- `test_monitor_speaker_layout_rust` — outside this eight-suite batch.
- `test_monitor_video_sync_rust` — outside this eight-suite batch.
- `test_monitor_wasapi_steps_rust` — outside this eight-suite batch.
- `test_nsgif_decode_rust` — outside this eight-suite batch.
- `test_pulse_monitor_steps_rust` — outside this eight-suite batch.
- `test_scene_audio_window_steps_rust` — outside this eight-suite batch.
- `test_scene_bounds_place_rust` — outside this eight-suite batch.
- `test_scene_item_crop_rust` — outside this eight-suite batch.
- `test_scene_item_names_steps_rust` — outside this eight-suite batch.
- `test_scene_item_order_steps_rust` — outside this eight-suite batch.
- `test_scene_item_transform_steps_rust` — outside this eight-suite batch.
- `test_scene_render_plan_steps_rust` — outside this eight-suite batch.
- `test_scene_transform_update_steps_rust` — outside this eight-suite batch.
- `test_source_audio_place_steps_rust` — outside this eight-suite batch.
- `test_source_color_space_steps_rust` — outside this eight-suite batch.
- `test_source_draw_plan_steps_rust` — outside this eight-suite batch.
- `test_source_duplicate_steps_rust` — outside this eight-suite batch.
- `test_source_filter_chain_steps_rust` — outside this eight-suite batch.
- `test_source_media_admission_steps_rust` — outside this eight-suite batch.
- `test_source_monitoring_steps_rust` — outside this eight-suite batch.
- `test_source_render_route_steps_rust` — outside this eight-suite batch.
- `test_source_video_tick_steps_rust` — outside this eight-suite batch.
- `test_sprite_geometry_steps_rust` — outside this eight-suite batch.
- `test_transition_clock_steps_rust` — outside this eight-suite batch.
- `test_transition_load_steps_rust` — outside this eight-suite batch.
- `test_transition_sample_time_steps_rust` — outside this eight-suite batch.
- `test_transition_start_steps_rust` — outside this eight-suite batch.
- `test_window_match_steps_rust` — outside this eight-suite batch.
- `test_windows_keys_steps_rust` — outside this eight-suite batch.

## Evidence

RED registration commit: `5308d0811`, failing with "missing Rust-static source
steps" before the graph additions. GREEN and compiler evidence are recorded
in the worker response. Native macOS/Windows runtime and final-head hybrid /
C-reference broad Tier 2/export parity are not claimed by this Linux archive
contract. Existing Rust unit/oracle parity and network tests remain unchanged;
this change introduces no implementation port or protocol behavior.

Focused graph/sorted-list/static-link guards and the whole-graph legacy replacement
check pass. Scoped Application-category Clippy with `libobs-integration` passes;
functions remain under the existing cognitive-complexity limit with no allowance.
The registration check passed 25 debug observations: longest 0.295 s, peak child
RSS 14,936 KiB, no test-owned threads. These deterministic checks do not use the
parity sampling setting. The native compilation contract retains the existing
named compilation-budget exception and starts compiler/CMake subprocesses.
Full workspace lint/unittest and broad OFF/ON Tier 2 were not run for this focused
branch-only batch; no target pruning, soldr gc, workflow changes or hosted CI.

GCC 15.2.0 and Clang 21.1.8 each pass both final module tests: eight real
Cargo-graph executable runs plus eight CMake/CTest executable runs per compiler.
Both consumers use the existing `target/release/liblibobs_rust.a` production
archive; no Rust implementation changed in this slice. Each compiler also passes
the archive OFF/ON inventory toggles and absent-timeout check. Independent primary
read-only review reports clean, including the fixture and helper corrections.
