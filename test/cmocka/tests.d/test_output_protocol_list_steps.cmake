# Characterization of the output protocol-list match in
# libobs/obs-output-protocols.c before its port: how many ';'-separated
# segments of a service output's protocols string equal the protocol. Builds
# the moved file on its own: no display and no libobs core, so it runs on
# every platform. The function is hidden in libobs, so this characterizes
# that C source; the test calls only it, so the port can run the same file
# against the Rust staticlib (as test_scene_bounds_place_rust does).
# test_output_protocols covers the caller through the real libobs.
add_executable(
  test_output_protocol_list_steps
  test_output_protocol_list_steps.c
  "${CMAKE_SOURCE_DIR}/libobs/obs-output-protocols.c.h"
)
target_include_directories(
  test_output_protocol_list_steps
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(
  test_output_protocol_list_steps
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES}
)
obs_test_sanitize(test_output_protocol_list_steps "address,undefined")

add_dependencies(cmocka-tests test_output_protocol_list_steps)
add_test(test_output_protocol_list_steps ${CMAKE_CURRENT_BINARY_DIR}/test_output_protocol_list_steps)
