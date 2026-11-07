# Production dock-state selection + real config safe-save/reopen (#3740).
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    add_executable(test_dock_state_restart test_dock_state_restart.cpp)
    target_include_directories(
      test_dock_state_restart
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
    )
    target_link_libraries(test_dock_state_restart PRIVATE OBS::libobs Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_dock_state_restart)
    add_test(NAME test_dock_state_restart COMMAND test_dock_state_restart)
    set_tests_properties(test_dock_state_restart PROPERTIES TIMEOUT 120 RESOURCE_LOCK timing)
  endif()
endif()
