# Characterization of the async video GPU conversion plan of
# libobs/obs-source.c (get_convert_type(), the set_*_sizes() of
# init_gpu_conversion(), select_conversion_technique() and
# need_linear_output()), moved to libobs/obs-source-conversion-plan.c before
# its port: the whole table, every video format and two values outside the
# enum, both ranges, every transfer and two values outside the enum, at sizes
# 0 to 3, odd, 1920 x 1080 and the uint32_t wraps near UINT32_MAX. Builds the
# moved file into itself. The function is hidden in libobs, so this
# characterizes that C source; the test calls only it.
# test_media_restart_first_frame, test_deinterlace_2x_field_order and
# test_pipewire_camera_nv12 cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shim. The test
# calls only the hidden obs_source_conversion_plan_fill() and includes only its
# header, graphics/graphics.h and media-io/video-io.h (for the enum values;
# none of them needs SIMDe or obsconfig.h), so it links the archive alone: its
# obs-source member needs nothing from libobs or the other crates. Linking the
# archive directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_async_conversion_plan_rust test_async_conversion_plan.c)
  target_include_directories(test_async_conversion_plan_rust PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_async_conversion_plan_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_async_conversion_plan_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_async_conversion_plan_rust)
  add_test(test_async_conversion_plan_rust ${CMAKE_CURRENT_BINARY_DIR}/test_async_conversion_plan_rust)
endif()
