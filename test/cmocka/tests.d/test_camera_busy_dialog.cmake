# Camera-busy warning window (obs-rust/obs-studio#3724): Close it, Ignore,
# Always allow, Escape and the window close each answer exactly once, nothing
# answers without a click, and the window is a non-modal child of the main
# window that never stays on top. Builds the real OBSCameraBusy (its own unity
# group, frontend/build/) without the frontend and runs it on Qt's offscreen
# platform, so it needs Qt Widgets and Qt Test but no display.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    set(_cb_dir "${CMAKE_SOURCE_DIR}/frontend")
    add_executable(
      test_camera_busy_dialog
      test_camera_busy_dialog.cpp
      "${_cb_dir}/build/frontend.dialogs-obscamerabusy+.hpp.cpp"
      "${_cb_dir}/dialogs/OBSCameraBusy.hpp"
    )
    set_target_properties(test_camera_busy_dialog PROPERTIES AUTOMOC ON)
    target_include_directories(test_camera_busy_dialog PRIVATE ${CMOCKA_INCLUDE_DIR} "${_cb_dir}" "${CMAKE_SOURCE_DIR}")
    target_link_libraries(test_camera_busy_dialog PRIVATE Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_camera_busy_dialog)
    obs_test_sanitize(test_camera_busy_dialog address,undefined)
    add_test(test_camera_busy_dialog ${CMAKE_CURRENT_BINARY_DIR}/test_camera_busy_dialog)
    # Qt and fontconfig keep allocations until exit; only memory errors count.
    set_tests_properties(test_camera_busy_dialog PROPERTIES ENVIRONMENT "ASAN_OPTIONS=detect_leaks=0")
    unset(_cb_dir)
  endif()
endif()
