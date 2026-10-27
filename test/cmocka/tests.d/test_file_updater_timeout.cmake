# file-updater bounds its HTTP request so an unreachable update host cannot
# hold shutdown through update_info_destroy's join
# (obs-rust/obs-studio#2672). Uses a blackholed TEST-NET address; Linux only
# like the other network-touching tests.
if(OS_LINUX)
  if(NOT TARGET OBS::file-updater)
    add_subdirectory("${CMAKE_SOURCE_DIR}/shared/file-updater" "${CMAKE_BINARY_DIR}/shared/file-updater")
  endif()
  add_executable(test_file_updater_timeout test_file_updater_timeout.c)
  target_include_directories(test_file_updater_timeout PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
  target_link_libraries(test_file_updater_timeout PRIVATE OBS::libobs OBS::file-updater ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_file_updater_timeout)
  add_test(test_file_updater_timeout ${CMAKE_CURRENT_BINARY_DIR}/test_file_updater_timeout)
  set_tests_properties(test_file_updater_timeout PROPERTIES TIMEOUT 200)
endif()
