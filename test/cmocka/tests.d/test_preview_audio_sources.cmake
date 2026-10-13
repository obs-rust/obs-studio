# Studio Mode Audio Mixer: audio sources inside groups and nested scenes of
# the previewed scene are marked as Preview (obsproject/obs-studio#13297).
# Builds the frontend helper the mixer calls against real libobs scenes;
# needs Qt Core but no frontend build. Its cases skip without a display
# (obs_startup), like test_filter_clipboard.
# Linux only, like test_output_timer: the macOS and Windows runners have no
# Qt runtime on the test rpath, and the cases need a display.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
  if(TARGET Qt6::Core)
    add_executable(
      test_preview_audio_sources
      test_preview_audio_sources.cpp
      "${CMAKE_SOURCE_DIR}/frontend/utility/preview-audio-sources.cpp"
    )
    target_include_directories(
      test_preview_audio_sources
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
    )
    target_link_libraries(test_preview_audio_sources PRIVATE OBS::libobs Qt6::Core ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_preview_audio_sources)
    add_test(test_preview_audio_sources ${CMAKE_CURRENT_BINARY_DIR}/test_preview_audio_sources)
  endif()
endif()
