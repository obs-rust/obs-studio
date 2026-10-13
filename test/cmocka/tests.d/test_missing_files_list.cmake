# --disable-missing-files-check leaks the missing files list of the loaded
# scene collection (obsproject/obs-studio#11074). Builds the plain C helper
# OBSBasic::LoadData() calls, so the test does not link the Qt frontend. It
# needs no video; obs_startup() needs a display on Linux, so there the cases
# that load sources skip without one.
add_executable(test_missing_files_list test_missing_files_list.c "${CMAKE_SOURCE_DIR}/frontend/utility/missing-files-list.c")
target_include_directories(test_missing_files_list PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_missing_files_list PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_missing_files_list)
add_test(test_missing_files_list ${CMAKE_CURRENT_BINARY_DIR}/test_missing_files_list)
