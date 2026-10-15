# A Scroll filter must show a source wider or taller than the largest GPU
# texture (OBS forum "long text file scrolling. Text disappears";
# obsproject/obs-studio#10034). Loads the built obs-filters module. Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET obs-filters AND TARGET OBS::libobs-opengl)
  add_executable(test_scroll_filter_wide_source test_scroll_filter_wide_source.c)
  target_compile_definitions(
    test_scroll_filter_wide_source
    PRIVATE
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      FILTERS_MODULE_PATH="$<TARGET_FILE:obs-filters>"
      FILTERS_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-filters/data"
  )
  target_include_directories(test_scroll_filter_wide_source PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scroll_filter_wide_source PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_scroll_filter_wide_source obs-filters libobs-opengl)
  add_dependencies(cmocka-tests test_scroll_filter_wide_source)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scroll_filter_wide_source
    COMMAND test_scroll_filter_wide_source
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scroll_filter_wide_source PROPERTIES TIMEOUT 120)
endif()
