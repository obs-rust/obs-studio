cmake_minimum_required(VERSION 3.28...3.30)

include(cmake/obs-version.cmake)

if(OS_WINDOWS AND NOT OBS_PARENT_ARCHITECTURE STREQUAL CMAKE_VS_PLATFORM_NAME)
  include(cmake/os-windows.cmake)
  return()
endif()

include(cmake/obs-core-modules.cmake)

find_package(SIMDe REQUIRED)
find_package(Threads REQUIRED)

find_package(FFmpeg 8.0 REQUIRED avformat avutil swscale swresample OPTIONAL_COMPONENTS avcodec)

find_package(ZLIB REQUIRED)
find_package(Uthash REQUIRED)

find_package(jansson REQUIRED)
if(NOT TARGET OBS::caption)
  add_subdirectory("${CMAKE_SOURCE_DIR}/deps/libcaption" "${CMAKE_BINARY_DIR}/deps/libcaption")
endif()

# The public/installed target is the hybrid. Build the C reference only when
# explicitly requested by migration tests (or select C for temporary rollback).
include(cmake/implementation.cmake)
obs_libobs_create_targets()

include(cmake/sources.cmake)

set_target_properties(libobs-objects PROPERTIES POSITION_INDEPENDENT_CODE TRUE)
target_sources(libobs-objects PRIVATE ${LIBOBS_SOURCES})
# MSVC /W4 (#2626): uthash's HASH_* macros end in uthash_fatal -> exit(-1),
# whose non-returning path makes MSVC report C4702 "unreachable code" at the
# closing brace of every HASH_ADD/HASH_ITER use (obs.c.h, obs-data.c.h,
# obs-hotkey.c.h, ...). The C oracle already suppresses C4702 for the same
# macros in its own build script, so this follows the vendored-code exemption
# (#516) instead of editing uthash or sprinkling pragmas in first-party code.
# Every C target needs it: the ported groups are also compiled by the
# C-reference library, which has its own compile options.
#
# Do not name a crate here: ci/test_tier2_proof_inputs.py treats any crate name
# mentioned in a cmake or CMakeLists.txt file as a Tier 2 root.
foreach(target libobs-objects ${OBS_LIBOBS_C_TARGETS})
  target_compile_options(${target} PRIVATE $<$<C_COMPILER_ID:MSVC>:/wd4702>)
endforeach()
obs_libobs_add_c_sources(${LIBOBS_PORTED_SOURCES})

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
# The shared libraries compile their own host sources, and the C reference
# compiles the same groups, so every libobs target carries it.
foreach(target libobs-objects ${LIBOBS_SHARED_TARGETS} ${OBS_LIBOBS_C_TARGETS})
  target_compile_options(
    ${target}
    PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
  )
endforeach()

# libnsgif is vendored C89 (the upstream NetSurf GIF decoder, kept as the
# differential oracle for the Rust port), so it keeps the per-source exemption
# the rest of the vendored C already has (librtmp, uthash): its conversions are
# not call sites this port edits.
set_property(
  SOURCE
    build/libobs.ported-nsgif-blocks+.h.c
    build/libobs.ported-nsgif-frame+.h.c
    build/libobs.ported-nsgif-gif+.h.c
    build/libobs.ported-nsgif-lzw+.h.c
  APPEND
  PROPERTY COMPILE_OPTIONS "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wno-conversion;-Wno-sign-conversion>"
)

# MSVC warns about INFINITY (a standard library macro) in obs-audio-controls.c
# (#870; was a pragma in the file).
set_property(
  SOURCE build/libobs.ported-audio-controls+.h.c
  APPEND
  PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4056;/wd4756>"
)
# libobs-version and libobs-core-modules are object libraries too, and CMake
# adds an object library's objects only to targets that link it directly, so
# each shared library links them itself (libobs-objects gets their headers).
foreach(target IN LISTS LIBOBS_SHARED_TARGETS)
  target_link_libraries(${target} PRIVATE libobs-objects OBS::libobs-core-modules OBS::libobs-version)
endforeach()

# Temporarily disables deprecation warnings while obs_data_autoselect_* is deprecated.
if(CMAKE_C_COMPILER_ID STREQUAL "MSVC")
  set_source_files_properties(build/libobs.ported-data+.h.c PROPERTIES COMPILE_OPTIONS "/wd4996")
elseif(
  CMAKE_C_COMPILER_ID STREQUAL "GNU"
  OR CMAKE_C_COMPILER_ID STREQUAL "Clang"
  OR CMAKE_C_COMPILER_ID STREQUAL "AppleClang"
)
  set_source_files_properties(build/libobs.ported-data+.h.c PROPERTIES COMPILE_OPTIONS "-Wno-deprecated-declarations")
endif()

# Usage requirements on libobs-objects are PUBLIC so they reach both shared
# libraries' own sources and link lines; the ones plugins also need are
# repeated on libobs's interface below.
target_compile_features(libobs-objects PUBLIC cxx_std_17)
target_compile_features(libobs INTERFACE cxx_std_17)

target_compile_definitions(
  libobs-objects
  PUBLIC
    IS_LIBOBS
    $<BUILD_INTERFACE:$<$<BOOL:${ENABLE_HEVC}>:ENABLE_HEVC>>
    $<BUILD_INTERFACE:$<$<BOOL:${ENABLE_FFMPEG_MUX_DEBUG}>:SHOW_SUBPROCESSES>>
)
target_compile_definitions(
  libobs
  INTERFACE
    $<BUILD_INTERFACE:$<$<BOOL:${ENABLE_HEVC}>:ENABLE_HEVC>>
    $<BUILD_INTERFACE:$<$<BOOL:${ENABLE_FFMPEG_MUX_DEBUG}>:SHOW_SUBPROCESSES>>
)

target_link_libraries(
  libobs-objects
  PUBLIC
    OBS::caption
    OBS::libobs-core-modules
    OBS::libobs-version
    FFmpeg::avcodec
    FFmpeg::avformat
    FFmpeg::avutil
    FFmpeg::swscale
    FFmpeg::swresample
    jansson::jansson
    Uthash::Uthash
    ZLIB::ZLIB
    SIMDe::SIMDe
    Threads::Threads
)
target_link_libraries(libobs INTERFACE SIMDe::SIMDe Threads::Threads)

if(OBS_LIBOBS_RUST_TARGET)
  find_program(OBS_LIBOBS_CARGO cargo REQUIRED)
  target_sources(${OBS_LIBOBS_RUST_TARGET} PRIVATE ${LIBOBS_RUST_SOURCES})
  include(cmake/rust.cmake)
endif()

if(OS_WINDOWS)
  include(cmake/os-windows.cmake)
elseif(OS_MACOS)
  include(cmake/os-macos.cmake)
elseif(OS_LINUX)
  include(cmake/os-linux.cmake)
elseif(OS_FREEBSD OR OS_OPENBSD)
  include(cmake/os-freebsd.cmake)
endif()

configure_file(obsconfig.h.in "${CMAKE_BINARY_DIR}/config/obsconfig.h" @ONLY)

foreach(target libobs-objects libobs)
  target_include_directories(
    ${target}
    PUBLIC "$<BUILD_INTERFACE:${CMAKE_BINARY_DIR}/config>" "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>"
  )
endforeach()

set(
  public_headers
  callback/calldata.h
  callback/decl.h
  callback/proc.h
  callback/signal.h
  graphics/axisang.h
  graphics/bounds.h
  graphics/effect-parser.h
  graphics/effect.h
  graphics/graphics.h
  graphics/image-file.h
  graphics/input.h
  graphics/libnsgif/nsgif.h
  graphics/math-defs.h
  graphics/math-extra.h
  graphics/matrix3.h
  graphics/matrix4.h
  graphics/plane.h
  graphics/quat.h
  graphics/shader-parser.h
  graphics/srgb.h
  graphics/vec2.h
  graphics/vec3.h
  graphics/vec4.h
  media-io/audio-io.h
  media-io/audio-math.h
  media-io/audio-resampler.h
  media-io/format-conversion.h
  media-io/frame-rate.h
  media-io/media-io-defs.h
  media-io/media-remux.h
  media-io/video-frame.h
  media-io/video-io.h
  media-io/video-scaler.h
  obs-audio-controls.h
  obs-avc.h
  obs-config.h
  obs-data.h
  obs-defs.h
  obs-encoder.h
  obs-hotkey.h
  obs-hotkeys.h
  obs-interaction.h
  obs-missing-files.h
  obs-module.h
  obs-nal.h
  obs-nix-platform.h
  obs-output.h
  obs-properties.h
  obs-service.h
  obs-source.h
  obs.h
  obs.hpp
  util/array-serializer.h
  util/base.h
  util/bitstream.h
  util/bmem.h
  util/c99defs.h
  util/cf-lexer.h
  util/cf-parser.h
  util/config-file.h
  util/crc32.h
  util/darray.h
  util/deque.h
  util/dstr.h
  util/dstr.hpp
  util/file-serializer.h
  util/lexer.h
  util/pipe.h
  util/platform.h
  util/profiler.h
  util/profiler.hpp
  util/serializer.h
  util/sse-intrin.h
  util/task.h
  util/text-lookup.h
  util/threading-posix.h
  util/threading.h
  util/uthash.h
  util/util.hpp
  util/util_uint128.h
  util/util_uint64.h
)

if(ENABLE_HEVC)
  list(APPEND public_headers obs-hevc.h)
endif()

set_property(TARGET libobs APPEND PROPERTY OBS_PUBLIC_HEADERS ${public_headers})

set_target_properties_obs(
  libobs
  PROPERTIES FOLDER core
             VERSION 0
             SOVERSION "${OBS_VERSION_MAJOR}"
)

target_export(libobs)

# The opt-in C reference takes the production file name and ABI version, which each
# platform's set_target_properties_obs decides (Linux: SOVERSION 30, not the
# OBS major version), so it loads in libobs's place (#1065).
if(TARGET libobs-c-reference)
  foreach(
    property
    VERSION
    SOVERSION
    OUTPUT_NAME
    PREFIX
    FRAMEWORK
    FRAMEWORK_VERSION
    MACOSX_FRAMEWORK_IDENTIFIER
    INSTALL_NAME_DIR
    BUILD_WITH_INSTALL_NAME_DIR
  )
    get_target_property(value libobs ${property})
    if(property STREQUAL "OUTPUT_NAME" AND value MATCHES "-NOTFOUND$")
      # No OUTPUT_NAME (macOS): the file name is the target name, libobs.
      set(value libobs)
    endif()
    if(NOT value MATCHES "-NOTFOUND$")
      set_property(TARGET libobs-c-reference PROPERTY ${property} "${value}")
    endif()
  endforeach()
  set_target_properties(
    libobs-c-reference
    PROPERTIES
      LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/libobs-c-reference"
      RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/libobs-c-reference"
      ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/libobs-c-reference"
  )
endif()
