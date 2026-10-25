project(obs-cmocka)

# 1.1: Debian 13 and Ubuntu 24.04 ship 1.1.7. cmocka-compat.h maps the 2.x
# names that the tests use (#2197).
find_package(CMocka 1.1 CONFIG REQUIRED)
add_library(obs-native-test-cmocka INTERFACE)
add_library(OBS::native-test-cmocka ALIAS obs-native-test-cmocka)
target_include_directories(obs-native-test-cmocka INTERFACE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(obs-native-test-cmocka INTERFACE ${CMOCKA_LIBRARIES})
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
# test_serializer is defined in build/test-graph.json.

# darray test
# test_darray is defined in build/test-graph.json.

# bitstream test
# test_bitstream is defined in build/test-graph.json.

# OS path test
# test_os_path is defined in build/test-graph.json.

# Product name shown by the frontend (frontend/utility/product-name.c), built
# without Qt.
add_executable(test_product_name test_product_name.c "${CMAKE_SOURCE_DIR}/frontend/utility/product-name.c.h")
target_include_directories(test_product_name PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_product_name PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_product_name)
add_test(test_product_name ${CMAKE_CURRENT_BINARY_DIR}/test_product_name)

# Exercise hotkey loading with an isolated core and no platform input thread.
# test_hotkey_load is defined in build/test-graph.json.

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

  # Registers accepted and rejected source, output, encoder and service types
  # while it loads (obs-rust/obs-studio#1953).
  add_library(core_module_stub_registers MODULE core_module_stub.c)
  target_compile_definitions(core_module_stub_registers PRIVATE CORE_STUB_REGISTERS)
  target_link_libraries(core_module_stub_registers PRIVATE OBS::libobs)
  set_target_properties(core_module_stub_registers PROPERTIES PREFIX "")

  add_executable(test_core_module_load test_core_module_load.c)
  target_include_directories(test_core_module_load PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_compile_definitions(
    test_core_module_load
    PRIVATE
      CORE_STUB_PATH="$<TARGET_FILE:core_module_stub>"
      CORE_STUB_NEEDS_DEP_PATH="$<TARGET_FILE:core_module_stub_needs_dep>"
      CORE_STUB_REGISTERS_PATH="$<TARGET_FILE:core_module_stub_registers>"
  )
  target_link_libraries(test_core_module_load PRIVATE OBS::libobs OBS::libobs-core-modules ${CMOCKA_LIBRARIES})
  add_dependencies(test_core_module_load core_module_stub core_module_stub_needs_dep core_module_stub_registers)
  add_dependencies(cmocka-tests test_core_module_load)
  add_test(test_core_module_load ${CMAKE_CURRENT_BINARY_DIR}/test_core_module_load)
  set_tests_properties(test_core_module_load PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()

# /dev/full reproduces delayed disk-write errors without fault injection.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  # test_config_save is defined in build/test-graph.json.

  # test_file_serializer_safe is defined in build/test-graph.json.

  # test_quick_write is defined in build/test-graph.json.
endif()

# AV1 OBU helpers (obs-av1.c)
# test_av1 is defined in build/test-graph.json.

# NAL start-code search (obs-nal.c)
# test_nal is defined in build/test-graph.json.

# AVC header test. Where the toolchain supports it, obs-avc.c is compiled into
# the test with UBSan (no recovery) so undefined behavior fails the test
# (obs-rust/obs-studio#11); its definitions take precedence over libobs's.
# test_avc is defined in build/test-graph.json.

# AVC helpers through libobs only, so ENABLE_RUST_LIBOBS=ON tests the port
# (test_avc above compiles its own obs-avc.c where UBSan is available).
# test_avc_parse is defined in build/test-graph.json.

# HEVC helpers (obs-hevc.c, built only with ENABLE_HEVC)
if(ENABLE_HEVC)
  # test_hevc is defined in build/test-graph.json.
endif()

# Formatted filename test (obs-rust/obs-studio#21). platform-filename.c is
# compiled into the test so the test's obs_get_video_info and time fixtures
# replace libobs's and libc's and no obs core has to be started; its definitions
# take precedence over libobs's.
# test_formatted_filename is defined in build/test-graph.json.

# CRC32 test
# test_crc32 is defined in build/test-graph.json.

# UTF-8 test. utf8.c is not exported by libobs, so it is compiled into the test
# (non-Windows only: the Windows implementation wraps the Win32 API).
# test_utf8 is defined in build/test-graph.json.

# dstr test
# test_dstr is defined in build/test-graph.json.

# dstr regression tests (obs-rust/obs-studio#46, #47)
# test_dstr_regressions is defined in build/test-graph.json.

# lexer test
# test_lexer is defined in build/test-graph.json.

# cf lexer test
# test_cf_lexer is defined in build/test-graph.json.

# text lookup test
# test_text_lookup is defined in build/test-graph.json.

# base (logging) test
# test_base is defined in build/test-graph.json.

# bmem test
# test_bmem is defined in build/test-graph.json.

# file serializer test (file-serializer.c, buffered-file-serializer.c)
# test_file_serializer is defined in build/test-graph.json.

# config file test (config-file.c)
# test_config_file is defined in build/test-graph.json.

# threading test (threading-posix.c, threading-windows.c)
# test_threading is defined in build/test-graph.json.

# Concurrent reads and writes of the private video-stat storage (#175).
# The sanitizer lane compiles this test with TSan to detect data races.
# test_video_stats_race is defined in build/test-graph.json.

# Qt-free concurrency check for the frontend meter's locked snapshot (#131).
add_executable(test_volume_meter_snapshot test_volume_meter_snapshot.cpp)
if(NOT MSVC)
  target_link_libraries(test_volume_meter_snapshot PRIVATE Threads::Threads)
endif()
add_dependencies(cmocka-tests test_volume_meter_snapshot)
add_test(test_volume_meter_snapshot ${CMAKE_CURRENT_BINARY_DIR}/test_volume_meter_snapshot)

# task test (task.c)
# test_task is defined in build/test-graph.json.

# pipe test (pipe.c, pipe-posix.c, pipe-windows.c)
# test_pipe is defined in build/test-graph.json.

# profiler test (profiler.c, source-profiler.c)
# test_profiler is defined in build/test-graph.json.

# platform test (platform.c, platform-nix*.c, platform-windows.c)
# test_platform is defined in build/test-graph.json.

# os_dtostr exponent normalization (obs-rust/obs-studio#754)
# test_dtostr_exponent is defined in build/test-graph.json.

# quat test (graphics/quat.h inline setters)
# test_quat is defined in build/test-graph.json.

# axisang test (graphics/axisang.c)
# test_axisang is defined in build/test-graph.json.

# vec2 test (graphics/vec2.c)
# test_vec2 is defined in build/test-graph.json.

# vec3 test (graphics/vec3.c)
# test_vec3 is defined in build/test-graph.json.

# vec4 test (graphics/vec4.c)
# test_vec4 is defined in build/test-graph.json.

# video fourcc test (media-io/video-fourcc.c)
# test_video_fourcc is defined in build/test-graph.json.

# video matrices test (media-io/video-matrices.c)
# test_video_matrices is defined in build/test-graph.json.

# video-io.h and frame-rate.h header-inline helpers
# test_video_io_inlines is defined in build/test-graph.json.

# util_uint64.h header-inline helper
# test_util_uint64 is defined in build/test-graph.json.

# Exercise AV1 encoder configuration against real FFmpeg options without a
# running OBS video session, then encode a small synthetic sequence with SVT.
find_package(FFmpeg 8.0 REQUIRED avcodec avformat avutil)
# test_svt_av1 is defined in build/test-graph.json.

# Exercise AMF decoding and RTMP server messages through the same
# authentication-enabled sources as obs-outputs, without starting OBS or
# connecting to a server.
# RTMP, AMF and FLV tests are defined in build/test-graph.json.

# Load .cube LUTs through the Color Grade filter's loader.
# test_cube_lut defined in build/test-graph.json.

# Linux capture reconnection after a PipeWire restart (obs-rust/obs-studio#108).
# The decisions are compiled into the tests directly; no PipeWire daemon,
# portal or PulseAudio server is needed.
# test_pipewire_reconnect defined in build/test-graph.json.

# test_pulse_reconnect and test_pulse_default_id defined in build/test-graph.json.

# video frame test (media-io/video-frame.c + video-frame.h inlines)
# test_video_frame is defined in build/test-graph.json.

include("${CMAKE_CURRENT_SOURCE_DIR}/qt-runtime.cmake")

# obs-ffmpeg-url.h includes its host's obs-ffmpeg-net-host.h (#853); every
# test that compiles obs-ffmpeg sources finds it here.
include_directories("${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/hosts/$<IF:$<PLATFORM_ID:Windows>,windows,posix>")
# net-if.h (obs-outputs) includes its host's net-if-os.h (#851); every
# test that compiles obs-outputs sources finds it here.
include_directories("${CMAKE_SOURCE_DIR}/plugins/obs-outputs/hosts/$<IF:$<PLATFORM_ID:Windows>,windows,posix>")

# One file per test from here on (obs-rust/obs-studio#137): add
# tests.d/<test>.cmake instead of appending to this file, so concurrent PRs
# that add tests do not conflict. Included in this directory's scope, so
# relative source paths and CMAKE_CURRENT_SOURCE_DIR are test/cmocka.
file(GLOB _cmocka_test_fragments CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/tests.d/*.cmake")
list(SORT _cmocka_test_fragments)
foreach(_fragment IN LISTS _cmocka_test_fragments)
  include("${_fragment}")
endforeach()
include("${CMAKE_CURRENT_SOURCE_DIR}/native-test-graph.cmake")
# Match the existing plugin-target guards independently of core feature options.
foreach(
  _graph_plugin
  IN
  ITEMS ffmpeg filters pipewire pulseaudio outputs qsv11
)
  string(TOUPPER "${_graph_plugin}" _graph_plugin_upper)
  if(TARGET OBS::${_graph_plugin})
    set(HAVE_OBS_${_graph_plugin_upper} TRUE)
  else()
    set(HAVE_OBS_${_graph_plugin_upper} FALSE)
  endif()
endforeach()

# These explicit false defaults retain the original if() semantics on hosts
# where a platform-only option is not declared.
foreach(
  _graph_fact
  IN
  ITEMS MSVC OS_LINUX OS_FREEBSD OS_OPENBSD OS_WINDOWS OS_MACOS ENABLE_PULSEAUDIO ENABLE_HEVC
)
  if(NOT DEFINED ${_graph_fact})
    set(${_graph_fact} FALSE)
  endif()
endforeach()
set(HAVE_OBS_OPENGL FALSE)
if(TARGET OBS::libobs-opengl)
  set(HAVE_OBS_OPENGL TRUE)
endif()
set(HAVE_VLC_MODULE FALSE)
if(TARGET vlc-video)
  get_target_property(_graph_vlc_type vlc-video TYPE)
  if(_graph_vlc_type STREQUAL "MODULE_LIBRARY")
    set(HAVE_VLC_MODULE TRUE)
  endif()
endif()
set(C_COMPILER_CLANG FALSE)
if(CMAKE_C_COMPILER_ID MATCHES "^(AppleClang|Clang)$")
  set(C_COMPILER_CLANG TRUE)
endif()
if(CMAKE_SYSTEM_NAME STREQUAL "OpenBSD")
  set(OS_OPENBSD TRUE)
endif()
if(OS_MACOS)
  foreach(_framework IN ITEMS Foundation Security)
    string(TOLOWER "${_framework}" _framework_lower)
    add_library(native-test-${_framework_lower} INTERFACE)
    add_library(OBS::native-test-${_framework_lower} ALIAS native-test-${_framework_lower})
    target_link_libraries(native-test-${_framework_lower} INTERFACE "$<LINK_LIBRARY:FRAMEWORK,${_framework}.framework>")
  endforeach()
endif()
obs_native_test_graph("${CMAKE_CURRENT_SOURCE_DIR}/build/test-graph.json" "${CMAKE_SOURCE_DIR}")
include("${CMAKE_CURRENT_SOURCE_DIR}/parallel-environment.cmake")
