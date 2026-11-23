if(OS_LINUX AND TARGET obs-vst3)
  find_package(Qt6 REQUIRED Core)
  set(_vst3_home "${CMAKE_CURRENT_BINARY_DIR}/vst3-test-home")
  set(_vst3_bundle "${_vst3_home}/.vst3/OBSFixture.vst3/Contents/x86_64-linux")

  add_library(vst3-test-gain MODULE vst3-test-gain.cpp)
  target_link_libraries(vst3-test-gain PRIVATE OBS::vst3-sdk)
  set_target_properties(
    vst3-test-gain
    PROPERTIES PREFIX "" OUTPUT_NAME OBSFixture LIBRARY_OUTPUT_DIRECTORY "${_vst3_bundle}"
  )

  add_executable(test_vst3_host test_vst3_host.c vst3-test-qt.cpp)
  target_link_libraries(test_vst3_host PRIVATE OBS::libobs Qt::Core)
  # The host starts the scanner beside the executable, as in a packaged build.
  add_dependencies(test_vst3_host obs-vst3 vst3-test-gain)
  add_custom_command(
    TARGET test_vst3_host
    POST_BUILD
    COMMAND
      "${CMAKE_COMMAND}" -E copy_if_different "$<TARGET_FILE:obs-vst3-scanner>" "$<TARGET_FILE_DIR:test_vst3_host>"
    VERBATIM
  )
  add_test(
    NAME test_vst3_host
    COMMAND
      "${CMAKE_COMMAND}" -E env "HOME=${_vst3_home}" "$<TARGET_FILE:test_vst3_host>" "$<TARGET_FILE:obs-vst3>"
      "${CMAKE_SOURCE_DIR}/plugins/obs-vst3/data" "${_vst3_home}" 57B71384C2EE4713B586F2FD7FDB08A1
      6BD4700D163F4219A265882F52B373E8
  )
  set_tests_properties(test_vst3_host PROPERTIES TIMEOUT 15)
  add_dependencies(cmocka-tests test_vst3_host)
endif()
