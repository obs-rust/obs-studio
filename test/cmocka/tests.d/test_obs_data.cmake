# obs-data.c. Pins the current C behavior. The port must not edit this test.
# probe
add_executable(test_obs_data test_obs_data.c)
target_include_directories(test_obs_data PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_obs_data PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_obs_data)
add_test(test_obs_data ${CMAKE_CURRENT_BINARY_DIR}/test_obs_data)
