# The "Proceed anyway" dialog the frontend shows when an RTMPS stream fails to
# connect (obs-rust/obs-studio#2205): a three-second countdown that accepts on
# its own, Cancel, and the stderr/log line. Builds the real dialog on Qt's
# offscreen platform, so it needs Qt Widgets but no display and no frontend
# build.
find_package(Qt6 QUIET COMPONENTS Widgets)
if(TARGET Qt6::Widgets AND NOT OS_WINDOWS)
  add_executable(
    test_rtmps_fallback_dialog
    test_rtmps_fallback_dialog.cpp
    "${CMAKE_SOURCE_DIR}/frontend/build/frontend.dialogs-obsrtmpsfallback+.hpp.cpp"
    "${CMAKE_SOURCE_DIR}/frontend/dialogs/OBSRtmpsFallback.hpp"
  )
  set_target_properties(test_rtmps_fallback_dialog PROPERTIES AUTOMOC ON)
  target_include_directories(
    test_rtmps_fallback_dialog
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/dialogs"
  )
  target_link_libraries(test_rtmps_fallback_dialog PRIVATE OBS::libobs Qt6::Widgets ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_rtmps_fallback_dialog)
  add_test(test_rtmps_fallback_dialog ${CMAKE_CURRENT_BINARY_DIR}/test_rtmps_fallback_dialog)
  set_tests_properties(test_rtmps_fallback_dialog PROPERTIES TIMEOUT 60)
  cmocka_qt_runtime(test_rtmps_fallback_dialog Qt6::Core)
endif()
