# A Color Correction filter at neutral settings must leave a high-precision
# (GS_CS_SRGB_16F) source as it is, not turn its out-of-gamut colours black
# (upstream obsproject/obs-studio#10403). Loads the built obs-filters module.
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET obs-filters AND TARGET OBS::libobs-opengl)
  add_executable(test_color_correction_16f test_color_correction_16f.c)
  target_compile_definitions(
    test_color_correction_16f
    PRIVATE
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      FILTERS_MODULE_PATH="$<TARGET_FILE:obs-filters>"
      FILTERS_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-filters/data"
  )
  target_include_directories(test_color_correction_16f PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_color_correction_16f PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_color_correction_16f obs-filters libobs-opengl)
  add_dependencies(cmocka-tests test_color_correction_16f)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_color_correction_16f
    COMMAND test_color_correction_16f
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_color_correction_16f PROPERTIES TIMEOUT 120)
endif()
