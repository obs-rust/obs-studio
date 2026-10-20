# An audio-only filter in a source's filter chain passes colour space
# queries on to the filter or source below it (upstream
# obsproject/obs-studio PR #13761). It needs no video; obs_startup() needs a
# display on Linux, so there the cases skip without one.
add_executable(test_filter_color_space test_filter_color_space.c)
target_include_directories(test_filter_color_space PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_filter_color_space PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_filter_color_space)
add_test(test_filter_color_space ${CMAKE_CURRENT_BINARY_DIR}/test_filter_color_space)
