include("${CMAKE_CURRENT_LIST_DIR}/../write-if-different.cmake")
# Characterize the original C body before its behavior-neutral extraction.
set(_tex_active_source "${CMAKE_SOURCE_DIR}/libobs/obs-encoder.c.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_tex_active_source}")
file(READ "${_tex_active_source}" _tex_active_text)
string(FIND "${_tex_active_text}" "bool obs_encoder_video_tex_active(" _tex_active_start)
if(_tex_active_start LESS 0)
  message(FATAL_ERROR "Cannot locate obs_encoder_video_tex_active")
endif()
string(SUBSTRING "${_tex_active_text}" ${_tex_active_start} -1 _tex_active_tail)
string(FIND "${_tex_active_tail}" "\n}\n" _tex_active_end)
math(EXPR _tex_active_length "${_tex_active_end} + 3")
string(SUBSTRING "${_tex_active_tail}" 0 ${_tex_active_length} _tex_active_body)
obs_test_write_file(
  "${CMAKE_CURRENT_BINARY_DIR}/encoder-video-tex-active-under-test.c"
  "#include \"libobs/build/libobs.pch.h\"\n${_tex_active_body}"
)
add_executable(
  test_encoder_video_tex_active
  test_encoder_video_tex_active.c
  "${CMAKE_CURRENT_BINARY_DIR}/encoder-video-tex-active-under-test.c"
)
target_compile_definitions(test_encoder_video_tex_active PRIVATE get_mix_for_video=tex_active_recording_mix_lookup)
target_link_libraries(
  test_encoder_video_tex_active
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_video_tex_active PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_video_tex_active)
add_test(NAME test_encoder_video_tex_active COMMAND test_encoder_video_tex_active)
