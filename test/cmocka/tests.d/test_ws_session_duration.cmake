# obs-websocket's settings dialog shows each client's Session Duration as
# hh:mm:ss (obsproject/obs-websocket#1208: the cell went empty after 59
# seconds). Builds the plugin's formatting helper straight from its header
# with Qt Core only: no libobs, no display, and the plugin itself may be off
# (Tier 2).
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(NOT TARGET Qt6::Core)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
if(TARGET Qt6::Core AND EXISTS "${_ws_src}/forms/SessionDuration.h")
  add_executable(test_ws_session_duration test_ws_session_duration.cpp)
  target_include_directories(test_ws_session_duration PRIVATE ${CMOCKA_INCLUDE_DIR} "${_ws_src}")
  target_link_libraries(test_ws_session_duration PRIVATE Qt6::Core ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_ws_session_duration)
  add_test(test_ws_session_duration ${CMAKE_CURRENT_BINARY_DIR}/test_ws_session_duration)
  cmocka_qt_runtime(test_ws_session_duration Qt6::Core)
endif()
unset(_ws_src)
