# Copy Transform and Paste Transform ignore items inside groups
# (obsproject/obs-studio#5478, obsproject/obs-studio#9558). Builds the plain C
# helper the frontend calls, so the test does not link the Qt frontend. Items
# inside a group are updated on video ticks, so this needs core video and
# runs (under xvfb-run) on Linux only; the cases skip without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_transform_clipboard
    test_transform_clipboard.c
    "${CMAKE_SOURCE_DIR}/frontend/utility/transform-clipboard.c"
  )
  target_compile_definitions(test_transform_clipboard PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_transform_clipboard
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
  )
  target_link_libraries(test_transform_clipboard PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_transform_clipboard libobs-opengl)
  add_dependencies(cmocka-tests test_transform_clipboard)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_transform_clipboard
    COMMAND test_transform_clipboard
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_transform_clipboard PROPERTIES TIMEOUT 120)
endif()
