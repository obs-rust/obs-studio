# Hiding a source that is only in the Studio Mode Preview must hide it
# (obsproject/obs-studio#4120). Hiding an item with audio is applied as an audio
# action, which needs core video and audio, so it runs (under xvfb-run) on Linux
# only.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_preview_only_hide test_preview_only_hide.c)
  target_compile_definitions(test_preview_only_hide PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_preview_only_hide PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_preview_only_hide PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_preview_only_hide libobs-opengl)
  add_dependencies(cmocka-tests test_preview_only_hide)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_preview_only_hide COMMAND test_preview_only_hide WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_preview_only_hide PROPERTIES TIMEOUT 120)
endif()
