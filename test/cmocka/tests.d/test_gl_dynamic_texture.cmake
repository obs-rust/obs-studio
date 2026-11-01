# A libobs-opengl dynamic texture's map, write and unmap hands the exact bytes
# to the next draw, without re-specifying the texture on every unmap (#3191).
# Runs the real GL module under xvfb-run on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_gl_dynamic_texture test_gl_dynamic_texture.c)
  target_include_directories(test_gl_dynamic_texture PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_compile_definitions(test_gl_dynamic_texture PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_link_libraries(test_gl_dynamic_texture PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_gl_dynamic_texture libobs-opengl)
  add_dependencies(cmocka-tests test_gl_dynamic_texture)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_gl_dynamic_texture COMMAND test_gl_dynamic_texture WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_gl_dynamic_texture PROPERTIES TIMEOUT 120)
endif()
