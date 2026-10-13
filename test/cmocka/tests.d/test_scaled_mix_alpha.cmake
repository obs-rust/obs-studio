# A rescaled output must show the same picture as the program where the
# canvas has colour but no alpha, as Subtract blending leaves it
# (obsproject/obs-studio#10746). Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scaled_mix_alpha test_scaled_mix_alpha.c)
  target_compile_definitions(test_scaled_mix_alpha PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_scaled_mix_alpha PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scaled_mix_alpha PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_scaled_mix_alpha libobs-opengl)
  add_dependencies(cmocka-tests test_scaled_mix_alpha)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_scaled_mix_alpha COMMAND test_scaled_mix_alpha WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_scaled_mix_alpha PROPERTIES TIMEOUT 120)
endif()
