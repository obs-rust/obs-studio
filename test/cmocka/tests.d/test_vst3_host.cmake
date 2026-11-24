if(OS_LINUX AND TARGET obs-vst3)
  find_package(Qt6 REQUIRED Widgets)
  include("${CMAKE_CURRENT_LIST_DIR}/../vst3-test-targets.cmake")
  set(_vst3_home "${CMAKE_CURRENT_BINARY_DIR}/vst3-test-home")
  set(_vst3_bundle "${_vst3_home}/.vst3/OBSFixture.vst3/Contents/x86_64-linux")

  set_target_properties(vst3-test-gain PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${_vst3_bundle}")

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

  if(TARGET obs-vst3-c-reference)
    add_dependencies(test_vst3_host obs-vst3-c-reference)
    add_test(
      NAME test_vst3_host_cpp_reference
      COMMAND
        "${CMAKE_COMMAND}" -E env "HOME=${_vst3_home}" "$<TARGET_FILE:test_vst3_host>"
        "$<TARGET_FILE:obs-vst3-c-reference>" "${CMAKE_SOURCE_DIR}/plugins/obs-vst3/data" "${_vst3_home}"
        57B71384C2EE4713B586F2FD7FDB08A1 6BD4700D163F4219A265882F52B373E8
    )
    set_tests_properties(test_vst3_host_cpp_reference PROPERTIES TIMEOUT 15)
    set_tests_properties(test_vst3_host test_vst3_host_cpp_reference PROPERTIES RESOURCE_LOCK vst3_host_cache)

    foreach(probe IN ITEMS test-vst3-variable-cpp test-vst3-variable-rust)
      add_dependencies("${probe}" vst3-test-gain)
      add_dependencies(test_vst3_host "${probe}")
    endforeach()
    add_test(
      NAME test_vst3_host_variable
      COMMAND
        "${CMAKE_COMMAND}" "-DCPP_HOST=$<TARGET_FILE:test-vst3-variable-cpp>"
        "-DRUST_HOST=$<TARGET_FILE:test-vst3-variable-rust>" "-DFIXTURE=${_vst3_home}/.vst3/OBSFixture.vst3" -P
        "${CMAKE_CURRENT_LIST_DIR}/../vst3-test-variable.cmake"
    )
    set_tests_properties(test_vst3_host_variable PROPERTIES TIMEOUT 15)
  endif()

  if(TARGET obs-vst3-scanner-c-reference)
    add_dependencies(test_vst3_host obs-vst3-scanner-c-reference)
    add_test(
      NAME test_vst3_scanner_parity
      COMMAND
        "${CMAKE_COMMAND}" "-DRUST_SCANNER=$<TARGET_FILE:obs-vst3-scanner>"
        "-DCPP_SCANNER=$<TARGET_FILE:obs-vst3-scanner-c-reference>" "-DHOME_PATH=${_vst3_home}"
        "-DCACHE_DIR=${CMAKE_CURRENT_BINARY_DIR}/vst3-scanner-parity" -P
        "${CMAKE_CURRENT_LIST_DIR}/../vst3_scanner_parity.cmake"
    )
    set_tests_properties(test_vst3_scanner_parity PROPERTIES TIMEOUT 15 FIXTURES_SETUP vst3_scanner_parity)
    set_tests_properties(test_vst3_host PROPERTIES FIXTURES_REQUIRED vst3_scanner_parity)
  endif()
endif()
