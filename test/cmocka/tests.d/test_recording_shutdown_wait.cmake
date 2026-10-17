# Quitting OBS while recording waits for the recording to stop without
# freezing the UI: the wait runs the Qt event loop on the output's stop
# signals, with the old timeout (#1283). Tests the frontend's header-only
# WaitForOutputToStop() against a fake output; needs QtCore, no display.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
  if(TARGET Qt6::Core)
    add_executable(test_recording_shutdown_wait test_recording_shutdown_wait.cpp)
    target_include_directories(
      test_recording_shutdown_wait
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/widgets"
    )
    target_link_libraries(test_recording_shutdown_wait PRIVATE OBS::libobs Qt6::Core ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_recording_shutdown_wait)
    add_test(test_recording_shutdown_wait ${CMAKE_CURRENT_BINARY_DIR}/test_recording_shutdown_wait)
    set_tests_properties(test_recording_shutdown_wait PROPERTIES TIMEOUT 60)
  endif()
endif()
