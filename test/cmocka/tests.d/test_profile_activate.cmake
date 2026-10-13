# Profile activation after a rename or duplicate (obsproject/obs-studio#12458).
# Builds the plain C helper the frontend calls against libobs' config_t, so
# the test needs neither Qt nor a display.
add_executable(test_profile_activate test_profile_activate.c "${CMAKE_SOURCE_DIR}/frontend/utility/profile-activate.c")
target_include_directories(test_profile_activate PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_profile_activate PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_profile_activate)
add_test(test_profile_activate ${CMAKE_CURRENT_BINARY_DIR}/test_profile_activate)
