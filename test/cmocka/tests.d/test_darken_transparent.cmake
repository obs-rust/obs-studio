# A source set to Darken blending must leave the background where it is
# transparent, not draw black there (obsproject/obs-studio#6591). Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_darken_transparent test_darken_transparent.c)
  target_compile_definitions(test_darken_transparent PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_darken_transparent PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_darken_transparent PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_darken_transparent libobs-opengl)
  add_dependencies(cmocka-tests test_darken_transparent)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_darken_transparent COMMAND test_darken_transparent WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_darken_transparent PROPERTIES TIMEOUT 120)
endif()
