# Captions queued on a video track whose codec cannot carry them (not h264,
# hevc or av1) must not leak the packet copy add_caption() makes. Drives a
# real output through the libobs that ships, as test_output_interleave_restart
# does, and reads the caption queue through obs-internal.h. It uses real core
# video, so it needs an X display on Linux (xvfb-run, as in CI).
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_output_caption_codec test_output_caption_codec.c)
  target_compile_definitions(test_output_caption_codec PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_output_caption_codec
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
  )
  target_link_libraries(test_output_caption_codec PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_output_caption_codec libobs-opengl)
  add_dependencies(cmocka-tests test_output_caption_codec)
  add_test(
    NAME test_output_caption_codec
    COMMAND test_output_caption_codec
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_output_caption_codec PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
endif()
