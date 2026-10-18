project(obs-cmocka)

find_package(CMocka CONFIG REQUIRED)
if(NOT MSVC)
  find_package(Threads REQUIRED)
endif()

# Every test executable joins this target, so the Tier 2 validators
# (rust/tools/*-validate) build `cmocka-tests` instead of a hand-kept list and a
# new test cannot go unbuilt (and show up in ctest as "Not Run").
add_custom_target(cmocka-tests)

# The display tests' add_test commands wrap themselves in xvfb-run, found as
# XVFB_RUN, XVFB_RUN_PROGRAM or XVFB_RUN_EXECUTABLE. Point all three at
# xvfb-run-or-display.sh, which reuses the display ctest already runs under
# instead of every test racing to start its own server (the fragments'
# find_program keeps a variable that is already set).
find_program(OBS_XVFB_RUN_REAL xvfb-run)
if(OBS_XVFB_RUN_REAL)
  configure_file(xvfb-run-or-display.sh.in "${CMAKE_CURRENT_BINARY_DIR}/xvfb-run-or-display.sh" @ONLY)
  file(
    CHMOD
    "${CMAKE_CURRENT_BINARY_DIR}/xvfb-run-or-display.sh"
    PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE
  )
  foreach(_xvfb_var IN ITEMS XVFB_RUN XVFB_RUN_PROGRAM XVFB_RUN_EXECUTABLE)
    set(${_xvfb_var} "${CMAKE_CURRENT_BINARY_DIR}/xvfb-run-or-display.sh" CACHE FILEPATH "" FORCE)
  endforeach()
endif()

# Existing C tests are never edited to satisfy a new warning, and they pass
# float to double varargs and math freely (as does the SWIG-generated code some
# of them compile), so -Wdouble-promotion (obs-rust/obs-studio#516) covers the
# product code, not the tests.
add_compile_options($<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-double-promotion>)
# Existing C tests are never edited to satisfy a new warning, and they cast
# const away freely, so -Wcast-qual (obs-rust/obs-studio#516) covers the
# product code, not the tests.
add_compile_options($<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-cast-qual>)
# Likewise -Wformat=2 (obs-rust/obs-studio#516) covers the product code only.
add_compile_options($<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-format-nonliteral>)
# Existing C tests are never edited (obs-rust/obs-studio#516).
add_compile_options($<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-shadow>)

# Per-test sanitizers (obs-rust/obs-studio#159). Some tests compile the code
# they check with ASan/UBSan so a bug fails the test. Those flags are added
# only here: never on MSVC, never when the whole build is already sanitized
# (mixing -fsanitize=thread with address does not build, and valgrind can't
# run ASan binaries), and not when OBS_TEST_SANITIZE_PER_TARGET is OFF.
# ci/test_cmocka_sanitize.py rejects raw -fsanitize flags anywhere else.
option(OBS_TEST_SANITIZE_PER_TARGET "Build selected cmocka tests with their own ASan/UBSan" ON)
function(obs_test_sanitize target sanitizers)
  if(MSVC OR NOT OBS_TEST_SANITIZE_PER_TARGET)
    return()
  endif()
  if("${CMAKE_C_FLAGS} ${CMAKE_CXX_FLAGS} ${CMAKE_EXE_LINKER_FLAGS}" MATCHES "-fsanitize=")
    return()
  endif()
  set(compile_options -fsanitize=${sanitizers} -fno-omit-frame-pointer)
  if(sanitizers MATCHES "undefined")
    list(APPEND compile_options -fno-sanitize-recover=undefined)
  endif()
  target_compile_options(${target} PRIVATE ${compile_options})
  target_link_options(${target} PRIVATE -fsanitize=${sanitizers})
endfunction()

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

# Product name shown by the frontend (frontend/utility/product-name.c), built
# without Qt.
add_executable(test_product_name test_product_name.c "${CMAKE_SOURCE_DIR}/frontend/utility/product-name.c.h")
target_include_directories(test_product_name PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_product_name PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_product_name)
add_test(test_product_name ${CMAKE_CURRENT_BINARY_DIR}/test_product_name)

# Exercise hotkey loading with an isolated core and no platform input thread.
add_executable(test_hotkey_load test_hotkey_load.c "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c.h")
target_compile_definitions(test_hotkey_load PRIVATE obs=hotkey_test_obs)
target_include_directories(test_hotkey_load PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_hotkey_load PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_hotkey_load)
add_test(test_hotkey_load ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_load)

# A core module that cannot be loaded (its file is missing, or a library it
# needs, like libcef.so for obs-browser, is not installed) must not stop OBS
# from loading the others or from starting (obs-rust/obs-studio#404). The test
# lays out stub modules under every core module name next to a copy of itself.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND TARGET OBS::libobs-core-modules)
  add_library(core_module_stub_dep SHARED core_module_stub_dep.c)
  set_target_properties(core_module_stub_dep PROPERTIES OUTPUT_NAME obs-test-missing-dep)

  add_library(core_module_stub MODULE core_module_stub.c)
  target_link_libraries(core_module_stub PRIVATE OBS::libobs)
  set_target_properties(core_module_stub PROPERTIES PREFIX "")

  # No RPATH: the dependency is out of reach at run time, as libcef.so was.
  add_library(core_module_stub_needs_dep MODULE core_module_stub.c)
  target_compile_definitions(core_module_stub_needs_dep PRIVATE CORE_STUB_NEEDS_DEP)
  target_link_libraries(core_module_stub_needs_dep PRIVATE OBS::libobs core_module_stub_dep)
  set_target_properties(core_module_stub_needs_dep PROPERTIES PREFIX "" SKIP_BUILD_RPATH TRUE)

  add_executable(test_core_module_load test_core_module_load.c)
  target_include_directories(test_core_module_load PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_compile_definitions(
    test_core_module_load
    PRIVATE
      CORE_STUB_PATH="$<TARGET_FILE:core_module_stub>"
      CORE_STUB_NEEDS_DEP_PATH="$<TARGET_FILE:core_module_stub_needs_dep>"
  )
  target_link_libraries(test_core_module_load PRIVATE OBS::libobs OBS::libobs-core-modules ${CMOCKA_LIBRARIES})
  add_dependencies(test_core_module_load core_module_stub core_module_stub_needs_dep)
  add_dependencies(cmocka-tests test_core_module_load)
  add_test(test_core_module_load ${CMAKE_CURRENT_BINARY_DIR}/test_core_module_load)
  set_tests_properties(test_core_module_load PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()

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
  target_sources(test_avc PRIVATE "${CMAKE_SOURCE_DIR}/libobs/obs-avc.c.h")
  obs_test_sanitize(test_avc undefined)
endif()

add_dependencies(cmocka-tests test_avc)
add_test(test_avc ${CMAKE_CURRENT_BINARY_DIR}/test_avc)

# AVC helpers through libobs only, so ENABLE_RUST_LIBOBS=ON tests the port
# (test_avc above compiles its own obs-avc.c where UBSan is available).
add_executable(test_avc_parse test_avc_parse.c)
target_include_directories(test_avc_parse PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_avc_parse PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_avc_parse)
add_test(test_avc_parse ${CMAKE_CURRENT_BINARY_DIR}/test_avc_parse)

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
    "${CMAKE_SOURCE_DIR}/libobs/util/platform.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/util/utf8.c.h"
  )
  target_include_directories(test_formatted_filename PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_formatted_filename PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  obs_test_sanitize(test_formatted_filename address,undefined)

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
  add_executable(test_utf8 test_utf8.c "${CMAKE_SOURCE_DIR}/libobs/util/utf8.c.h")
  target_include_directories(test_utf8 PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_utf8 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  obs_test_sanitize(test_utf8 address,undefined)

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

# Concurrent reads and writes of the private video-stat storage (#175).
# The sanitizer lane compiles this test with TSan to detect data races.
add_executable(test_video_stats_race test_video_stats_race.c)
target_link_libraries(test_video_stats_race PRIVATE OBS::libobs)
if(NOT MSVC)
  target_link_libraries(test_video_stats_race PRIVATE Threads::Threads)
endif()
add_dependencies(cmocka-tests test_video_stats_race)
add_test(test_video_stats_race ${CMAKE_CURRENT_BINARY_DIR}/test_video_stats_race)

# Qt-free concurrency check for the frontend meter's locked snapshot (#131).
add_executable(test_volume_meter_snapshot test_volume_meter_snapshot.cpp)
if(NOT MSVC)
  target_link_libraries(test_volume_meter_snapshot PRIVATE Threads::Threads)
endif()
add_dependencies(cmocka-tests test_volume_meter_snapshot)
add_test(test_volume_meter_snapshot ${CMAKE_CURRENT_BINARY_DIR}/test_volume_meter_snapshot)

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

# os_dtostr exponent normalization (obs-rust/obs-studio#754)
add_executable(test_dtostr_exponent test_dtostr_exponent.c)
target_include_directories(test_dtostr_exponent PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_dtostr_exponent PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_dtostr_exponent)
add_test(test_dtostr_exponent ${CMAKE_CURRENT_BINARY_DIR}/test_dtostr_exponent)

# quat test (graphics/quat.h inline setters)
add_executable(test_quat test_quat.c)
target_include_directories(test_quat PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_quat PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_quat PRIVATE m)
endif()

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

# video-io.h and frame-rate.h header-inline helpers
add_executable(test_video_io_inlines test_video_io_inlines.c)
target_include_directories(test_video_io_inlines PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_io_inlines PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_video_io_inlines)
add_test(test_video_io_inlines ${CMAKE_CURRENT_BINARY_DIR}/test_video_io_inlines)

# util_uint64.h header-inline helper
add_executable(test_util_uint64 test_util_uint64.c)
target_include_directories(test_util_uint64 PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_util_uint64 PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_util_uint64)
add_test(test_util_uint64 ${CMAKE_CURRENT_BINARY_DIR}/test_util_uint64)

# Exercise AV1 encoder configuration against real FFmpeg options without a
# running OBS video session, then encode a small synthetic sequence with SVT.
find_package(FFmpeg 8.0 REQUIRED avcodec avformat avutil)
add_executable(
  test_svt_av1
  test_svt_av1.c
  "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-video-encoders.c.h"
  "${CMAKE_SOURCE_DIR}/shared/opts-parser/opts-parser.c.h"
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

# Exercise AMF decoding and RTMP server messages through the same
# authentication-enabled sources as obs-outputs, without starting OBS or
# connecting to a server.
if(TARGET OBS::outputs)
  foreach(test_name IN ITEMS test_rtmp_error test_rtmp_chunk_size test_amf_decode)
    add_executable(
      ${test_name}
      ${test_name}.c
      "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/rtmp.c"
      "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/amf.c"
      "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/log.c"
      "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/parseurl.c"
      "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/hashswf.c"
    )
    target_compile_definitions(${test_name} PRIVATE USE_MBEDTLS CRYPTO)
    target_compile_options(
      ${test_name}
      PRIVATE
        $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-comma>
        $<$<AND:$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>,$<COMPILE_LANG_AND_ID:C,Clang>>:-Wno-error=unreachable-code>
    )
    target_include_directories(${test_name} PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs")
    target_link_libraries(
      ${test_name}
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
    add_dependencies(cmocka-tests ${test_name})
    add_test(${test_name} ${CMAKE_CURRENT_BINARY_DIR}/${test_name})
  endforeach()

  if(NOT MSVC)
    target_compile_options(test_amf_decode PRIVATE -fsigned-char)
    obs_test_sanitize(test_amf_decode undefined)
  endif()
endif()

# Build obs-outputs' FLV tags from flv-mux.c, without librtmp's network code.
if(TARGET OBS::outputs)
  add_executable(
    test_flv_mux
    test_flv_mux.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/flv-mux.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/amf.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/log.c"
  )
  # No network code is built, so librtmp's TLS types are not needed.
  target_compile_definitions(test_flv_mux PRIVATE NO_CRYPTO)
  target_include_directories(test_flv_mux PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs")
  target_link_libraries(test_flv_mux PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} $<$<PLATFORM_ID:Windows>:ws2_32>)
  add_dependencies(cmocka-tests test_flv_mux)
  add_test(test_flv_mux ${CMAKE_CURRENT_BINARY_DIR}/test_flv_mux)
endif()

# Load .cube LUTs through the Color Grade filter's loader.
if(TARGET OBS::filters)
  add_executable(test_cube_lut test_cube_lut.c "${CMAKE_SOURCE_DIR}/plugins/obs-filters/cube-lut.c.h")
  target_include_directories(test_cube_lut PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-filters")
  target_link_libraries(test_cube_lut PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} $<$<NOT:$<PLATFORM_ID:Windows>>:m>)
  add_dependencies(cmocka-tests test_cube_lut)
  add_test(test_cube_lut ${CMAKE_CURRENT_BINARY_DIR}/test_cube_lut)
endif()

# Linux capture reconnection after a PipeWire restart (obs-rust/obs-studio#108).
# The decisions are compiled into the tests directly; no PipeWire daemon,
# portal or PulseAudio server is needed.
if(TARGET OBS::pipewire)
  add_executable(
    test_pipewire_reconnect
    test_pipewire_reconnect.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c.h"
  )
  target_include_directories(
    test_pipewire_reconnect
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire"
  )
  target_link_libraries(test_pipewire_reconnect PRIVATE OBS::libobs PipeWire::PipeWire ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pipewire_reconnect)
  add_test(test_pipewire_reconnect ${CMAKE_CURRENT_BINARY_DIR}/test_pipewire_reconnect)
endif()

if(TARGET OBS::pulseaudio)
  add_executable(
    test_pulse_reconnect
    test_pulse_reconnect.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pulseaudio/pulse-reconnect.c.h"
  )
  target_include_directories(
    test_pulse_reconnect
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pulseaudio"
  )
  target_link_libraries(test_pulse_reconnect PRIVATE OBS::libobs PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pulse_reconnect)
  add_test(test_pulse_reconnect ${CMAKE_CURRENT_BINARY_DIR}/test_pulse_reconnect)

  # obs-rust/obs-studio#158: pulseaudio-wrapper.c against a fake libpulse
  # defined in the test (headers only, no libpulse link).
  add_executable(
    test_pulse_default_id
    test_pulse_default_id.c
    "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse/pulseaudio-wrapper.c.h"
  )
  target_include_directories(
    test_pulse_default_id
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse"
      $<TARGET_PROPERTY:PulseAudio::PulseAudio,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_link_libraries(test_pulse_default_id PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pulse_default_id)
  add_test(test_pulse_default_id ${CMAKE_CURRENT_BINARY_DIR}/test_pulse_default_id)
endif()

# video frame test (media-io/video-frame.c + video-frame.h inlines)
add_executable(test_video_frame test_video_frame.c)
target_include_directories(test_video_frame PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_frame PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_video_frame)
add_test(test_video_frame ${CMAKE_CURRENT_BINARY_DIR}/test_video_frame)

# cmocka_qt_runtime(<test> <Qt target>): lets a Qt-linked test find Qt at run
# time outside the app bundle, through the test's environment: macOS gets the
# directory holding the Qt frameworks on DYLD_FRAMEWORK_PATH (a build rpath
# does not reach the binary under the Xcode generator), Windows gets Qt's bin
# directory on PATH and its plugins directory (next to bin, as
# find_qt_plugins() expects) on QT_PLUGIN_PATH. Linux finds Qt through the
# system loader. The path is resolved here: tests added with
# add_test(<name> <cmd>) get no generator expressions in their properties.
function(cmocka_qt_runtime test qt_target)
  get_target_property(qt_location ${qt_target} LOCATION)
  if(NOT qt_location)
    return()
  endif()
  if(OS_MACOS)
    # .../lib/QtCore.framework/Versions/A/QtCore -> .../lib
    string(REGEX REPLACE "/[^/]+\\.framework/.*$" "" qt_dir "${qt_location}")
    set_property(
      TEST ${test}
      APPEND
      PROPERTY ENVIRONMENT_MODIFICATION "DYLD_FRAMEWORK_PATH=path_list_prepend:${qt_dir}"
    )
  elseif(OS_WINDOWS)
    # .../bin/Qt6Core.dll -> .../bin and .../plugins
    get_filename_component(qt_dir "${qt_location}" DIRECTORY)
    set_property(TEST ${test} APPEND PROPERTY ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:${qt_dir}")
    get_filename_component(qt_prefix "${qt_dir}" DIRECTORY)
    if(IS_DIRECTORY "${qt_prefix}/plugins")
      set_property(
        TEST ${test}
        APPEND
        PROPERTY ENVIRONMENT_MODIFICATION "QT_PLUGIN_PATH=path_list_prepend:${qt_prefix}/plugins"
      )
    endif()
  endif()
endfunction()

# One file per test from here on (obs-rust/obs-studio#137): add
# tests.d/<test>.cmake instead of appending to this file, so concurrent PRs
# that add tests do not conflict. Included in this directory's scope, so
# relative source paths and CMAKE_CURRENT_SOURCE_DIR are test/cmocka.
file(GLOB _cmocka_test_fragments CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/tests.d/*.cmake")
list(SORT _cmocka_test_fragments)
foreach(_fragment IN LISTS _cmocka_test_fragments)
  include("${_fragment}")
endforeach()
