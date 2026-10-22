# Characterization of the context name helpers in libobs/obs-context-names.c,
# ported to rust/obs-core (context_names): the "__unnamed%04lld" name and the
# " 2", " 3", ... suffix, against a scripted exists(). Builds the helpers on
# their own: no libobs core and no name tables. The functions are hidden in
# libobs, so this characterizes that C source. There is no _rust variant
# against libobs_rust-static: the shims' archive member also holds the rest
# of obs-core, which pulls in obs-media-io and obs-util (libuuid, GIO, and
# libobs's hidden base_default_log_handler), so it does not link on its own.
# The Tier 3 parity tests in rust/obs-core compare the shims with this C
# instead. test_context_names covers the callers through the real libobs.
add_executable(test_context_name_helpers test_context_name_helpers.c "${CMAKE_SOURCE_DIR}/libobs/obs-context-names.c.h")
target_include_directories(
  test_context_name_helpers
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(test_context_name_helpers PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_context_name_helpers "address,undefined")

add_dependencies(cmocka-tests test_context_name_helpers)
add_test(test_context_name_helpers ${CMAKE_CURRENT_BINARY_DIR}/test_context_name_helpers)
