# A second obs_output_start() while the output is still starting (a stream
# still connecting) ran the output's start() again and crashed rtmp_output
# (obsproject/obs-studio#13283). No video or encoders; obs_startup() needs a
# display on Linux, so there the cases skip without one.
add_executable(test_output_start_twice test_output_start_twice.c)
target_include_directories(test_output_start_twice PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_output_start_twice PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_output_start_twice)
add_test(test_output_start_twice ${CMAKE_CURRENT_BINARY_DIR}/test_output_start_twice)
