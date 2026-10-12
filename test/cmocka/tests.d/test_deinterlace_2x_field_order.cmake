# Double-rate deinterlacing must show the fields of an interlaced source in
# capture order for both field orders (upstream obsproject/obs-studio#6061),
# also when the field rate differs from the canvas rate
# (upstream obsproject/obs-studio#3402).
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_deinterlace_2x_field_order test_deinterlace_2x_field_order.c)
  target_compile_definitions(
    test_deinterlace_2x_field_order
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_deinterlace_2x_field_order PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_deinterlace_2x_field_order PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_deinterlace_2x_field_order libobs-opengl)
  add_dependencies(cmocka-tests test_deinterlace_2x_field_order)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_deinterlace_2x_field_order
    COMMAND test_deinterlace_2x_field_order
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_deinterlace_2x_field_order PROPERTIES TIMEOUT 300)
endif()
