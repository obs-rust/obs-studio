#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Cross-build the Windows MSVC Rust tests on Linux and run them under Wine (#5845).

    uv run --no-project ci/windows_wine_tests.py

`cargo test --target x86_64-pc-windows-msvc` builds with soldr's clang-cl
toolchain (the same graph as ci/windows_link_check.py) and hands each test
binary to ci/wine-runner.sh, which runs it headless. Tests that fail only
because Wine lacks an API are listed in WINE_SKIPS; the native
windows-2025-vs2026 lane still runs them on schedule, dispatch and master.
"""

from __future__ import annotations

import os
import sys

import local_gate

TARGET = "x86_64-pc-windows-msvc"

# Crates whose tests probe a real GPU/driver: obs-tests-gpu takes an unhandled
# page fault in Wine's graphics stack (the run crashes, no test result).
WINE_EXCLUDED_CRATES: tuple[str, ...] = ("obs-gpu-status",)

# Genuine Wine gaps only, as `--skip` filters, each with the reason (see #5845).
WINE_SKIPS: tuple[str, ...] = (
    # The test spawns the native Windows toolchain (cmake, ninja, cl) and gets
    # "program not found" under Wine: it needs a real Windows host (35 tests).
    "native_build::canonical_display_startup_is_registered::canonical_display_startup_declared_runner",
    "native_build::canonical_freetype_file_sdk_is_registered::canonical_freetype_file_sdk_absent_configures",
    "native_build::canonical_glyph_atlas_unavailable_hosts_configure_without_packages",
    "native_build::canonical_pipewire_cameras_is_registered::canonical_pipewire_cameras_cmake_unavailable",
    "native_build::canonical_plugin_sdk_headless_is_registered::canonical_plugin_sdk_unavailable_hosts_configure_without_packages",
    "native_build::canonical_portal_output_is_registered::portal_output_other_hosts_configure_without_daemon_sdks",
    "native_build::canonical_private_capture_is_registered::canonical_private_capture_cmake_unavailable",
    "native_build::canonical_qt_core_filename_is_registered::qt_core_filename_other_hosts_configure_without_optional_sdks",
    "native_build::canonical_qt_daemons_is_registered::canonical_qt_daemons_cmake_without_qt",
    "native_build::canonical_qt_daemons_is_registered::canonical_qt_daemons_frontend_owner_is_optional",
    "native_build::canonical_qt_widgets_is_registered::canonical_qt_widgets_cmake_without_qt",
    "native_build::canonical_texture_file_data_swap_is_registered::standalone_graphics_fixtures_bind_the_static_implementation",
    "native_build::canonical_texture_file_data_swap_is_registered::standalone_image_adapter_supplies_windows_host_imports",
    "native_build::canonical_v4l2_capture_sdk_is_registered::canonical_v4l2_capture_sdk_absent_configures",
    "native_build::canonical_v4l2_freetype_sdk_is_registered::canonical_v4l2_freetype_sdk_absent_configures",
    "native_build::canonical_video_info_query_is_registered::video_info_query_support_covers_all_rust_static_consumers",
    "native_build::canonical_wayland_sdk_is_registered::canonical_wayland_sdk_absent_configures",
    "native_build::canonical_websocket_helpers_is_registered::configured_headers::configured_headers_match_consumers_and_rebuild_after_template_or_value_changes",
    "native_build::executable_runs_against_its_shared_core_dependency",
    "native_build::explicit_groups_compile_as_individual_sources_and_observe_member_edits",
    "native_build::graph_host_configurations::native_test_graph_lua_watch_inputs_allow_ninja_frontend_configuration",
    "native_build::graph_host_configurations::native_test_graph_portable_startup_without_output_directory",
    "native_build::graph_host_configurations::native_test_graph_windows_slideshow_has_one_host_and_filetime_helper",
    "native_build::mixed_c_and_cpp_use_independent_flags_and_the_cpp_linker",
    "native_build::msvc_pch_links_its_object_and_preserves_language_header_and_flag_changes",
    "native_build::native_plugin_links_shared_core_and_observes_rebuilt_dependency",
    "native_build::native_profile_switch_rebuilds_assertion_policy",
    "native_build::native_test_graph_cmake_builds_and_runs_selected_contract",
    "native_build::native_test_graph_cmake_platform_requirements_allow_false_but_reject_unknown",
    "native_build::native_test_graph_conditional_prefixes_match_consumers",
    "native_build::native_test_graph_prepends_runtime_paths_in_order",
    "native_build::native_test_graph_preserves_cpp_standard_without_changing_c_compilation",
    "native_build::native_test_graph_preserves_include_precedence",
    "native_build::native_test_graph_rejects_colliding_qt_ui_outputs",
    "native_build::prebuilt::logical_recipe_keeps_native_cmake_env_execution",
    "native_build::unity_toggle_preserves_exports_and_explicit_generated_source_wins",
    "native_build::windows_resource_payload_tracks_headers_definitions_and_unity",
    # obs-source copy_filters differs from the C oracle under Wine (not root-caused).
    "source::copy_filters_parity::every_case_matches_c",
    # Flaky under Wine: "Sharing violation" (os error 32) on a temp file the test
    # reopens; passes on some runs.
    "obs_core::module_load_metadata_parity::differential_generated_fields",
    # Same "Sharing violation" on the final remove_dir_all, seen only on the
    # self-hosted Dell runners (run 46113979795); native Windows passes.
    "obs_core::module_load_metadata_parity::manifests_and_typed_defaults",
    # Wine on the self-hosted Dell runners: the C oracle's safe replace over an
    # existing file does not take effect (stays "old") and the Rust output is
    # NotFound after close (run 46113979795); hosted Wine and native Windows pass.
    "tests::serializer::file_serializer_parity::safe_ext_and_replace_match_oracle",
    "tests::serializer::file_serializer_parity::safe_output_matches_oracle",
    # Exit-watchdog helper: the hang-in-ExitProcess scenario differs under Wine's
    # loader (failure output not captured; not root-caused).
    "exit_watchdog::helper::windows::the_helper_ends_a_hang_in_exit_process",
    "exit_watchdog::helper::windows::the_helpers_line_names_the_dll",
    # Wine's ceilf/floorf quiet a signaling NaN (0x7f800001 becomes 0x7fc00001).
    "linear::round::edge_values",
    "linear::round::exhaustive_matches_libm",
    "linear::vec2_parity::ceil_floor_edge_values_match_c_oracle",
    # The client sends Host "[127.0.0.1]" where the test expects the name.
    "ipv6::rejects_brackets_around_non_ipv6_hosts",
    # Wine 10 (the CI runner) differs from the C oracle on UTF-8 conversion
    # capacity/size-query results and on the missing-binary error code.
    "tests::platform::pipe::windows_process::pipe_create2_missing_binary_mirrors_test_pipe_create2_missing_binary",
    "tests::platform::platform_conversions_parity::capacity_is_the_only_difference",
    "tests::platform::platform_conversions_parity::characterization_inputs_match_oracle",
    "tests::platform::platform_conversions_parity::utf8_to_wcs_matches_oracle",
    "tests::platform::platform_number_parity::strtod_matches_c_oracle",
    # Wine UCRT: wcstombs in the C locale converts an unconvertible char
    # (returns 2, not 0), and strftime %Z names the zone differently.
    "tests::platform::platform_conversions::test_wcs_to_mbs_c_locale",
    "tests::platform::platform_filename::zone_specifiers",
)


def test_args(target: str = TARGET) -> list[str]:
    args = ["test", "--workspace", "--locked", "--target", target, "--no-fail-fast"]
    args += [a for crate in WINE_EXCLUDED_CRATES for a in ("--exclude", crate)]
    if WINE_SKIPS:
        args += ["--", *[a for skip in WINE_SKIPS for a in ("--skip", skip)]]
    return args


def main() -> int:
    local_gate.add_target(TARGET)
    runner = local_gate.ROOT / "ci" / "wine-runner.sh"
    env = {f"CARGO_TARGET_{TARGET.upper().replace('-', '_')}_RUNNER": str(runner)}
    local_gate.soldr_cargo(test_args(), extra_env=env)
    return 0


if __name__ == "__main__":
    sys.exit(main())
