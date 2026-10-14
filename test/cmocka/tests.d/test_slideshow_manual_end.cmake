# An Image Slide Show in Manual mode with Playback Mode "Once" ends on its
# last slide instead of wrapping to the first on Next (forum thread 138392).
# Builds both slideshows (obs-slideshow-mk2.c, obs-slideshow.c) and the Cut
# transition directly so the image-source and obs-transitions modules are not
# loaded. Needs core video, so it runs (under xvfb-run) on Linux and skips
# without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_slideshow_manual_end
    test_slideshow_manual_end.c
    "${CMAKE_SOURCE_DIR}/plugins/image-source/obs-slideshow.c"
    "${CMAKE_SOURCE_DIR}/plugins/image-source/obs-slideshow-mk2.c"
    "${CMAKE_SOURCE_DIR}/plugins/image-source/slideshow-size.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-transitions/transition-cut.c"
  )
  target_compile_definitions(test_slideshow_manual_end PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_slideshow_manual_end
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/image-source"
  )
  target_link_libraries(test_slideshow_manual_end PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_slideshow_manual_end libobs-opengl)
  add_dependencies(cmocka-tests test_slideshow_manual_end)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_slideshow_manual_end
    COMMAND test_slideshow_manual_end
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_slideshow_manual_end PROPERTIES TIMEOUT 120)
endif()
