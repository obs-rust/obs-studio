# Fallback warning window (obs-rust/obs-studio#2218): the countdown, both
# buttons, Escape and the window close each end the dialog exactly once, with
# the right end value of the C ABI. Builds the real
# OBSFallbackWarning (its own unity group, frontend/build/) without the
# frontend and runs it on Qt's offscreen platform, so it needs Qt Widgets and
# Qt Test but no display.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    set(_fw_dir "${CMAKE_SOURCE_DIR}/frontend")
    add_executable(
      test_fallback_warning_dialog
      test_fallback_warning_dialog.cpp
      "${_fw_dir}/build/frontend.dialogs-obsfallbackwarning+.hpp.cpp"
      "${_fw_dir}/dialogs/OBSFallbackWarning.hpp"
    )
    set_target_properties(test_fallback_warning_dialog PROPERTIES AUTOMOC ON)
    target_include_directories(
      test_fallback_warning_dialog
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${_fw_dir}" "${CMAKE_SOURCE_DIR}"
    )
    target_link_libraries(test_fallback_warning_dialog PRIVATE Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_fallback_warning_dialog)
    obs_test_sanitize(test_fallback_warning_dialog address,undefined)
    add_test(test_fallback_warning_dialog ${CMAKE_CURRENT_BINARY_DIR}/test_fallback_warning_dialog)
    # Qt and fontconfig keep allocations until exit; only memory errors count.
    set_tests_properties(test_fallback_warning_dialog PROPERTIES ENVIRONMENT "ASAN_OPTIONS=detect_leaks=0")
    unset(_fw_dir)
  endif()
endif()
