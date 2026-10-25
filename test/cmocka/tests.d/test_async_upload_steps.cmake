# Characterization of the async video upload decisions of libobs/obs-source.c
# (set_async_texture_size(), update_async_texrender(), update_async_textures(),
# check_to_swap_bgrx_bgra() and obs_source_update_async_video()), moved to
# libobs/obs-source-async-upload.c and ported to rust/obs-source
# (async_upload): the five-field compare with each field changed alone, the
# GPU conversion and plain upload routes for every convert type, the size
# uniforms as float bits for sizes 0 to UINT32_MAX, the HLG peak clamp, the
# maximum nits and its ratio to SDR white levels of 80, 300 and 480 for the
# SRGB, PQ and HLG transfers and one outside the enum at peaks of 100, 1000,
# 1001 and 10000, hdr_lw, the colour range, the linear alpha flag, every BGR
# swap pair and the four re-anchor cases. Builds the moved file into itself.
# The functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. test_async_conversion_plan, test_pipewire_cursor_hdr,
# test_media_format_change and test_filter_color_space cover the callers
# through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the fourteen hidden functions and includes their header,
# obs-source-conversion-plan.h (header only) and obs.h (for the enum values;
# obs.h pulls SIMDe and obsconfig.h, hence libobs's include directories), so
# it links the archive alone: its obs-source member needs nothing from libobs
# or the other crates. Linking the archive directly avoids exporting private
# symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_async_upload_steps_rust test_async_upload_steps.c)
  target_include_directories(
    test_async_upload_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_async_upload_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_async_upload_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_async_upload_steps_rust)
  add_test(test_async_upload_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_async_upload_steps_rust)
endif()
