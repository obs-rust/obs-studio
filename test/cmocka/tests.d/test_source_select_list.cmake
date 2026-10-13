# Add Source dialog: the existing sources of a type are listed by name
# (obsproject/obs-studio#13947). Builds the frontend helper the dialog calls
# against real libobs sources; needs Qt Core but no frontend build. Its
# cases skip without a display (obs_startup), like test_filter_clipboard.
# Linux only, like test_output_timer: the macOS and Windows runners have no
# Qt runtime on the test rpath, and the cases need a display.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
  if(TARGET Qt6::Core)
    add_executable(
      test_source_select_list
      test_source_select_list.cpp
      "${CMAKE_SOURCE_DIR}/frontend/utility/source-select-list.cpp"
    )
    target_include_directories(
      test_source_select_list
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
    )
    target_link_libraries(test_source_select_list PRIVATE OBS::libobs Qt6::Core ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_source_select_list)
    add_test(test_source_select_list ${CMAKE_CURRENT_BINARY_DIR}/test_source_select_list)
  endif()
endif()
