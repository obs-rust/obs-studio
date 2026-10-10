project(obs-cmocka)

find_package(CMocka CONFIG REQUIRED)
if(NOT MSVC)
  find_package(Threads REQUIRED)
endif()

# Every test executable joins this target, so the Tier 2 validators
# (rust/tools/*-validate) build `cmocka-tests` instead of a hand-kept list and a
# new test cannot go unbuilt (and show up in ctest as "Not Run").
add_custom_target(cmocka-tests)

# Serializer test
add_executable(test_serializer test_serializer.c)
target_include_directories(test_serializer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_serializer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_serializer)
add_test(test_serializer ${CMAKE_CURRENT_BINARY_DIR}/test_serializer)

# darray test
add_executable(test_darray test_darray.c)
target_include_directories(test_darray PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_darray PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_darray)
add_test(test_darray ${CMAKE_CURRENT_BINARY_DIR}/test_darray)

# bitstream test
add_executable(test_bitstream test_bitstream.c)
target_include_directories(test_bitstream PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_bitstream PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_bitstream)
add_test(test_bitstream ${CMAKE_CURRENT_BINARY_DIR}/test_bitstream)

# OS path test
add_executable(test_os_path test_os_path.c)
target_include_directories(test_os_path PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_os_path PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_os_path)
add_test(test_os_path ${CMAKE_CURRENT_BINARY_DIR}/test_os_path)

# Exercise hotkey loading with an isolated core and no platform input thread.
add_executable(test_hotkey_load test_hotkey_load.c "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c")
target_compile_definitions(test_hotkey_load PRIVATE obs=hotkey_test_obs)
target_include_directories(test_hotkey_load PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_hotkey_load PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_hotkey_load)
add_test(test_hotkey_load ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_load)

# /dev/full reproduces delayed disk-write errors without fault injection.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  add_executable(test_config_save test_config_save.c)
  target_include_directories(test_config_save PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_config_save PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_config_save)
  add_test(test_config_save ${CMAKE_CURRENT_BINARY_DIR}/test_config_save)

  add_executable(test_file_serializer_safe test_file_serializer_safe.c)
  target_include_directories(test_file_serializer_safe PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_file_serializer_safe PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_file_serializer_safe)
  add_test(test_file_serializer_safe ${CMAKE_CURRENT_BINARY_DIR}/test_file_serializer_safe)

  add_executable(test_quick_write test_quick_write.c)
  target_include_directories(test_quick_write PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_quick_write PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_quick_write)
  add_test(test_quick_write ${CMAKE_CURRENT_BINARY_DIR}/test_quick_write)
endif()

# AV1 OBU helpers (obs-av1.c)
add_executable(test_av1 test_av1.c)
target_include_directories(test_av1 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_av1 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_av1)
add_test(test_av1 ${CMAKE_CURRENT_BINARY_DIR}/test_av1)

# NAL start-code search (obs-nal.c)
add_executable(test_nal test_nal.c)
target_include_directories(test_nal PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_nal PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_nal)
add_test(test_nal ${CMAKE_CURRENT_BINARY_DIR}/test_nal)

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

add_dependencies(cmocka-tests test_avc)
add_test(test_avc ${CMAKE_CURRENT_BINARY_DIR}/test_avc)

# HEVC helpers (obs-hevc.c, built only with ENABLE_HEVC)
if(ENABLE_HEVC)
  add_executable(test_hevc test_hevc.c)
  target_include_directories(test_hevc PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_hevc PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

  add_dependencies(cmocka-tests test_hevc)
  add_test(test_hevc ${CMAKE_CURRENT_BINARY_DIR}/test_hevc)
endif()

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

  add_dependencies(cmocka-tests test_formatted_filename)
  add_test(test_formatted_filename ${CMAKE_CURRENT_BINARY_DIR}/test_formatted_filename)
endif()

# CRC32 test
add_executable(test_crc32 test_crc32.c)
target_include_directories(test_crc32 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_crc32 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_crc32)
add_test(test_crc32 ${CMAKE_CURRENT_BINARY_DIR}/test_crc32)

# UTF-8 test. utf8.c is not exported by libobs, so it is compiled into the test
# (non-Windows only: the Windows implementation wraps the Win32 API).
if(NOT MSVC)
  add_executable(test_utf8 test_utf8.c "${CMAKE_SOURCE_DIR}/libobs/util/utf8.c")
  target_include_directories(test_utf8 PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_utf8 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  target_compile_options(test_utf8 PRIVATE -fsanitize=address,undefined -fno-sanitize-recover=undefined)
  target_link_options(test_utf8 PRIVATE -fsanitize=address,undefined)

  add_dependencies(cmocka-tests test_utf8)
  add_test(test_utf8 ${CMAKE_CURRENT_BINARY_DIR}/test_utf8)
endif()

# dstr test
add_executable(test_dstr test_dstr.c)
target_include_directories(test_dstr PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_dstr PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_dstr)
add_test(test_dstr ${CMAKE_CURRENT_BINARY_DIR}/test_dstr)

# dstr regression tests (obs-rust/obs-studio#46, #47)
add_executable(test_dstr_regressions test_dstr_regressions.c)
target_include_directories(test_dstr_regressions PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_dstr_regressions PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_dstr_regressions)
add_test(test_dstr_regressions ${CMAKE_CURRENT_BINARY_DIR}/test_dstr_regressions)
# A regression of #47 loops forever: fail fast instead.
set_tests_properties(test_dstr_regressions PROPERTIES TIMEOUT 60)

# lexer test
add_executable(test_lexer test_lexer.c)
target_include_directories(test_lexer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_lexer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_lexer)
add_test(test_lexer ${CMAKE_CURRENT_BINARY_DIR}/test_lexer)

# cf lexer test
add_executable(test_cf_lexer test_cf_lexer.c)
target_include_directories(test_cf_lexer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_cf_lexer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_cf_lexer)
add_test(test_cf_lexer ${CMAKE_CURRENT_BINARY_DIR}/test_cf_lexer)

# text lookup test
add_executable(test_text_lookup test_text_lookup.c)
target_include_directories(test_text_lookup PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_text_lookup PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_text_lookup)
add_test(test_text_lookup ${CMAKE_CURRENT_BINARY_DIR}/test_text_lookup)

# base (logging) test
add_executable(test_base test_base.c)
target_include_directories(test_base PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_base PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_base)
add_test(test_base ${CMAKE_CURRENT_BINARY_DIR}/test_base)

# bmem test
add_executable(test_bmem test_bmem.c)
target_include_directories(test_bmem PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_bmem PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_bmem)
add_test(test_bmem ${CMAKE_CURRENT_BINARY_DIR}/test_bmem)

# file serializer test (file-serializer.c, buffered-file-serializer.c)
add_executable(test_file_serializer test_file_serializer.c)
target_include_directories(test_file_serializer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_file_serializer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_file_serializer)
add_test(test_file_serializer ${CMAKE_CURRENT_BINARY_DIR}/test_file_serializer)

# config file test (config-file.c)
add_executable(test_config_file test_config_file.c)
target_include_directories(test_config_file PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_config_file PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_config_file)
add_test(test_config_file ${CMAKE_CURRENT_BINARY_DIR}/test_config_file)

# threading test (threading-posix.c, threading-windows.c)
add_executable(test_threading test_threading.c)
target_include_directories(test_threading PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_threading PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_threading PRIVATE Threads::Threads)
endif()

add_dependencies(cmocka-tests test_threading)
add_test(test_threading ${CMAKE_CURRENT_BINARY_DIR}/test_threading)

# task test (task.c)
add_executable(test_task test_task.c)
target_include_directories(test_task PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_task PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_task PRIVATE Threads::Threads)
endif()

add_dependencies(cmocka-tests test_task)
add_test(test_task ${CMAKE_CURRENT_BINARY_DIR}/test_task)

# pipe test (pipe.c, pipe-posix.c, pipe-windows.c)
add_executable(test_pipe test_pipe.c)
target_include_directories(test_pipe PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_pipe PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_pipe)
add_test(test_pipe ${CMAKE_CURRENT_BINARY_DIR}/test_pipe)

# profiler test (profiler.c, source-profiler.c)
add_executable(test_profiler test_profiler.c)
target_include_directories(test_profiler PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_profiler PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_profiler)
add_test(test_profiler ${CMAKE_CURRENT_BINARY_DIR}/test_profiler)

# platform test (platform.c, platform-nix*.c, platform-windows.c)
add_executable(test_platform test_platform.c)
target_include_directories(test_platform PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_platform PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_platform)
add_test(test_platform ${CMAKE_CURRENT_BINARY_DIR}/test_platform)

# quat test (graphics/quat.h inline setters)
add_executable(test_quat test_quat.c)
target_include_directories(test_quat PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_quat PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_quat)
add_test(test_quat ${CMAKE_CURRENT_BINARY_DIR}/test_quat)

# axisang test (graphics/axisang.c)
add_executable(test_axisang test_axisang.c)
target_include_directories(test_axisang PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_axisang PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_axisang PRIVATE m)
endif()

add_dependencies(cmocka-tests test_axisang)
add_test(test_axisang ${CMAKE_CURRENT_BINARY_DIR}/test_axisang)

# vec2 test (graphics/vec2.c)
add_executable(test_vec2 test_vec2.c)
target_include_directories(test_vec2 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_vec2 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_vec2 PRIVATE m)
endif()

add_dependencies(cmocka-tests test_vec2)
add_test(test_vec2 ${CMAKE_CURRENT_BINARY_DIR}/test_vec2)

# vec3 test (graphics/vec3.c)
add_executable(test_vec3 test_vec3.c)
target_include_directories(test_vec3 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_vec3 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_vec3 PRIVATE m)
endif()

add_dependencies(cmocka-tests test_vec3)
add_test(test_vec3 ${CMAKE_CURRENT_BINARY_DIR}/test_vec3)

# vec4 test (graphics/vec4.c)
add_executable(test_vec4 test_vec4.c)
target_include_directories(test_vec4 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_vec4 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_vec4 PRIVATE m)
endif()

add_dependencies(cmocka-tests test_vec4)
add_test(test_vec4 ${CMAKE_CURRENT_BINARY_DIR}/test_vec4)

# video fourcc test (media-io/video-fourcc.c)
add_executable(test_video_fourcc test_video_fourcc.c)
target_include_directories(test_video_fourcc PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_fourcc PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_video_fourcc)
add_test(test_video_fourcc ${CMAKE_CURRENT_BINARY_DIR}/test_video_fourcc)

# video matrices test (media-io/video-matrices.c)
add_executable(test_video_matrices test_video_matrices.c)
target_include_directories(test_video_matrices PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_matrices PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_video_matrices)
add_test(test_video_matrices ${CMAKE_CURRENT_BINARY_DIR}/test_video_matrices)

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
add_dependencies(cmocka-tests test_svt_av1)
add_test(test_svt_av1 ${CMAKE_CURRENT_BINARY_DIR}/test_svt_av1)
set_tests_properties(test_svt_av1 PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)

# Decode RTMP server errors through the same authentication-enabled sources
# as obs-outputs, without starting OBS or connecting to a server.
if(TARGET OBS::outputs)
  add_executable(
    test_rtmp_error
    test_rtmp_error.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/rtmp.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/amf.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/log.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/parseurl.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/hashswf.c"
  )
  target_compile_definitions(test_rtmp_error PRIVATE USE_MBEDTLS CRYPTO)
  target_compile_options(
    test_rtmp_error
    PRIVATE
      $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-comma>
      $<$<AND:$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>,$<COMPILE_LANG_AND_ID:C,Clang>>:-Wno-error=unreachable-code>
  )
  target_include_directories(test_rtmp_error PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs")
  target_link_libraries(
    test_rtmp_error
    PRIVATE
      OBS::libobs
      OBS::happy-eyeballs
      MbedTLS::mbedtls
      ZLIB::ZLIB
      ${CMOCKA_LIBRARIES}
      $<$<PLATFORM_ID:Windows>:crypt32>
      $<$<PLATFORM_ID:Windows>:iphlpapi>
      $<$<PLATFORM_ID:Windows>:winmm>
      $<$<PLATFORM_ID:Windows>:ws2_32>
      "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Foundation.framework>>"
      "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Security.framework>>"
  )
  add_dependencies(cmocka-tests test_rtmp_error)
  add_test(test_rtmp_error ${CMAKE_CURRENT_BINARY_DIR}/test_rtmp_error)
endif()

# Build obs-outputs' FLV tags from flv-mux.c, without librtmp's network code.
if(TARGET OBS::outputs)
  add_executable(
    test_flv_mux
    test_flv_mux.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/flv-mux.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/amf.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/log.c"
  )
  target_include_directories(test_flv_mux PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs")
  target_link_libraries(test_flv_mux PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} $<$<PLATFORM_ID:Windows>:ws2_32>)
  add_dependencies(cmocka-tests test_flv_mux)
  add_test(test_flv_mux ${CMAKE_CURRENT_BINARY_DIR}/test_flv_mux)
endif()
