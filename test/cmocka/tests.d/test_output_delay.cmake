# Characterization of the output stream delay in libobs/obs-output-delay.c
# before its port (#1546). process_delay() and the delay start, stop and
# cleanup are not exported, so the test compiles the file, with a zeroed
# struct obs_output, stubs for the obs-output.c functions it calls and a test
# clock: no libobs core and no display, so it runs on every platform. The
# file's precompiled header is libobs.pch.h, which needs the private libobs,
# libcaption, uthash and zlib headers.
# output_delay_fields_stub.c provides the delay field accessors of
# obs-output.c over the test's struct obs_output.
add_executable(test_output_delay test_output_delay.c output_delay_fields_stub.c)
target_include_directories(
  test_output_delay
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(
  test_output_delay
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ZLIB::ZLIB ${CMOCKA_LIBRARIES}
)

add_dependencies(cmocka-tests test_output_delay)
add_test(test_output_delay ${CMAKE_CURRENT_BINARY_DIR}/test_output_delay)
