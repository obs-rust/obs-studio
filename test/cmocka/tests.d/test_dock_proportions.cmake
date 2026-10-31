# Narrowing the main window and widening it again lost the docks' relative
# sizes: Qt shrinks every dock in a row to its minimum, then shares the space
# out equally (obs-rust/obs-studio#2225). Builds the frontend helper
# (frontend/utility/dock-proportions.hpp) and drives a QMainWindow with a row
# of docks on Qt's offscreen platform: needs Qt Widgets and Qt Test only, no
# display and no frontend build.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    add_executable(test_dock_proportions test_dock_proportions.cpp)
    target_include_directories(
      test_dock_proportions
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
    )
    target_link_libraries(test_dock_proportions PRIVATE Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_dock_proportions)
    add_test(test_dock_proportions ${CMAKE_CURRENT_BINARY_DIR}/test_dock_proportions)
    # Its checks depend on wall-clock timing: the asan pool runs it alone (#3238).
    set_tests_properties(test_dock_proportions PROPERTIES TIMEOUT 120 RESOURCE_LOCK timing)
  endif()
endif()
