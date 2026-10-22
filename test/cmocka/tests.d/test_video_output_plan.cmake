# Characterization of the video output plan in libobs/obs-video-plan.c before
# its port: the size check and alignment of obs_reset_video(), and for each
# output format the GPU conversion techniques, texel sizes, conversion planes
# and render texture format. Compiles the file into itself (C only), so it
# needs no libobs core and no display. libobs.pch.h needs the private libobs,
# libcaption and uthash headers.
add_executable(test_video_output_plan test_video_output_plan.c)
target_compile_definitions(
  test_video_output_plan
  PRIVATE FORMAT_CONVERSION_EFFECT="${CMAKE_SOURCE_DIR}/libobs/data/format_conversion.effect"
)
target_include_directories(
  test_video_output_plan
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(test_video_output_plan PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_video_output_plan "address,undefined")

add_dependencies(cmocka-tests test_video_output_plan)
add_test(test_video_output_plan ${CMAKE_CURRENT_BINARY_DIR}/test_video_output_plan)
