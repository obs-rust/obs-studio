# Exit in the tray menu did nothing when OBS started minimized to the tray on
# Wayland (obsproject/obs-studio#13494): the never-shown main window had no
# platform window, so QWidget::close() sent it no close event. Builds the
# frontend helper OBSBasic::close() uses (frontend/utility/close-top-level.hpp)
# and drives a top-level widget in that state on Qt's offscreen platform:
# needs Qt Widgets only, no display and no frontend build.
find_package(Qt6 QUIET COMPONENTS Widgets)
if(TARGET Qt6::Widgets)
  add_executable(test_close_top_level test_close_top_level.cpp)
  target_include_directories(test_close_top_level PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
  target_link_libraries(test_close_top_level PRIVATE Qt6::Widgets ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_close_top_level)
  add_test(test_close_top_level ${CMAKE_CURRENT_BINARY_DIR}/test_close_top_level)
  # Runs in well under a second; a stuck Qt start fails the test, not the CI job.
  set_tests_properties(test_close_top_level PROPERTIES TIMEOUT 120)
  cmocka_qt_runtime(test_close_top_level Qt6::Core)
endif()
