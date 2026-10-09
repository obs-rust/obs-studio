project(obs-cmocka)

find_package(CMocka CONFIG REQUIRED)

# Serializer test
add_executable(test_serializer test_serializer.c)
target_include_directories(test_serializer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_serializer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_test(test_serializer ${CMAKE_CURRENT_BINARY_DIR}/test_serializer)

# darray test
add_executable(test_darray test_darray.c)
target_include_directories(test_darray PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_darray PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_test(test_darray ${CMAKE_CURRENT_BINARY_DIR}/test_darray)

# bitstream test
add_executable(test_bitstream test_bitstream.c)
target_include_directories(test_bitstream PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_bitstream PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_test(test_bitstream ${CMAKE_CURRENT_BINARY_DIR}/test_bitstream)

# OS path test
add_executable(test_os_path test_os_path.c)
target_include_directories(test_os_path PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_os_path PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_test(test_os_path ${CMAKE_CURRENT_BINARY_DIR}/test_os_path)

# Exercise AV1 encoder configuration against real FFmpeg options without a
# running OBS video session, then encode a small synthetic sequence with SVT.
find_package(FFmpeg 8.0 REQUIRED avcodec avformat avutil)
add_executable(
  test_svt_av1
  test_svt_av1.c
  "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-video-encoders.c"
  "${CMAKE_SOURCE_DIR}/shared/opts-parser/opts-parser.c"
)
target_compile_definitions(
  test_svt_av1
  PRIVATE
    obs_encoder_video=svt_test_encoder_video
    video_output_get_info=svt_test_video_info
    obs_encoder_get_width=svt_test_encoder_size
    obs_encoder_get_height=svt_test_encoder_size
    obs_encoder_get_name=svt_test_encoder_name
    obs_encoder_set_last_error=svt_test_set_last_error
)
target_include_directories(test_svt_av1 PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/shared/opts-parser")
target_link_libraries(
  test_svt_av1
  PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
)
add_test(test_svt_av1 ${CMAKE_CURRENT_BINARY_DIR}/test_svt_av1)
set_tests_properties(test_svt_av1 PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)

# AVC header test. Where the toolchain supports it, obs-avc.c is compiled into
# the test with UBSan (no recovery) so undefined behavior fails the test
# (obs-rust/obs-studio#11); its definitions take precedence over libobs's.
add_executable(test_avc test_avc.c)
target_include_directories(test_avc PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_avc PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_sources(test_avc PRIVATE "${CMAKE_SOURCE_DIR}/libobs/obs-avc.c")
  target_compile_options(test_avc PRIVATE -fsanitize=undefined -fno-sanitize-recover=undefined)
  target_link_options(test_avc PRIVATE -fsanitize=undefined)
endif()

add_test(test_avc ${CMAKE_CURRENT_BINARY_DIR}/test_avc)

# Formatted filename test (obs-rust/obs-studio#21). platform.c is compiled into
# the test so the test's obs_get_video_info fixture replaces libobs's and no obs
# core has to be started; its definitions take precedence over libobs's. utf8.c
# is not exported by libobs, and platform.c needs it.
if(NOT MSVC)
  add_executable(
    test_formatted_filename
    test_formatted_filename.c
    "${CMAKE_SOURCE_DIR}/libobs/util/platform.c"
    "${CMAKE_SOURCE_DIR}/libobs/util/utf8.c"
  )
  target_include_directories(test_formatted_filename PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_formatted_filename PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  target_compile_options(test_formatted_filename PRIVATE -fsanitize=address,undefined -fno-sanitize-recover=undefined)
  target_link_options(test_formatted_filename PRIVATE -fsanitize=address,undefined)

  add_test(test_formatted_filename ${CMAKE_CURRENT_BINARY_DIR}/test_formatted_filename)
endif()
