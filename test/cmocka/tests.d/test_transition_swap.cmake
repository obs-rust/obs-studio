# Swapping the program transition mid-transition must hand the running
# transition over (upstream obsproject/obs-studio#11914). Needs core video, so
# it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_transition_swap test_transition_swap.c)
  target_compile_definitions(test_transition_swap PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_transition_swap PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_transition_swap PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_transition_swap libobs-opengl)
  add_dependencies(cmocka-tests test_transition_swap)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_transition_swap COMMAND test_transition_swap WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_transition_swap PROPERTIES TIMEOUT 60)
endif()
