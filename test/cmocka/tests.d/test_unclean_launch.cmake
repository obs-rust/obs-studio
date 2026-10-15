# After an unclean shutdown, --disable-shutdown-check must skip the modal
# "Crash Detected" dialog, which blocks unattended starts forever
# (obs-rust/obs-studio#809). Builds the Qt-free header OBSApp uses.
add_executable(test_unclean_launch test_unclean_launch.cpp)
target_include_directories(test_unclean_launch PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_compile_features(test_unclean_launch PRIVATE cxx_std_17)
target_link_libraries(test_unclean_launch PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_unclean_launch)
add_test(test_unclean_launch ${CMAKE_CURRENT_BINARY_DIR}/test_unclean_launch)
