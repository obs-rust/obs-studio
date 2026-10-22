# Characterization of the context name helpers in libobs/obs-context-names.c
# before their port: the "__unnamed%04lld" name and the " 2", " 3", ...
# suffix, against a scripted exists(). Builds the helpers on their own: no
# libobs core and no name tables. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them, so the port can run
# the same file against the Rust staticlib (as test_scene_bounds_place_rust
# does). test_context_names covers the callers through the real libobs.
add_executable(test_context_name_helpers test_context_name_helpers.c "${CMAKE_SOURCE_DIR}/libobs/obs-context-names.c.h")
target_include_directories(
  test_context_name_helpers
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(test_context_name_helpers PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_context_name_helpers "address,undefined")

add_dependencies(cmocka-tests test_context_name_helpers)
add_test(test_context_name_helpers ${CMAKE_CURRENT_BINARY_DIR}/test_context_name_helpers)
