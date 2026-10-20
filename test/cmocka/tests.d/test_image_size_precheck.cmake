# An image larger than the graphics device's texture size limit must fail
# before it is decoded, not after (obsproject/obs-studio#3427). Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_image_size_precheck test_image_size_precheck.c)
  target_compile_definitions(test_image_size_precheck PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_image_size_precheck PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_image_size_precheck PRIVATE OBS::libobs ZLIB::ZLIB ${CMOCKA_LIBRARIES})
  add_dependencies(test_image_size_precheck libobs-opengl)
  add_dependencies(cmocka-tests test_image_size_precheck)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_image_size_precheck
    COMMAND test_image_size_precheck
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_image_size_precheck PROPERTIES TIMEOUT 120)
endif()
