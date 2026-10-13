# Profile events when a profile is removed (obsproject/obs-websocket#1267).
# Builds the plain C helper the frontend calls, so the test needs neither Qt
# nor a display.
add_executable(test_profile_remove test_profile_remove.c "${CMAKE_SOURCE_DIR}/frontend/utility/profile-remove.c")
target_include_directories(test_profile_remove PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_profile_remove PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_profile_remove)
add_test(test_profile_remove ${CMAKE_CURRENT_BINARY_DIR}/test_profile_remove)
