cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_NVENC "Build NVIDIA Hardware Encoder Plugin" ON)
option(ENABLE_NVENC_FFMPEG_IDS "Register FFmpeg encoder IDs" ON)
mark_as_advanced(ENABLE_NVENC_FFMPEG_IDS)

if(NOT ENABLE_NVENC)
  target_disable_feature(obs-nvenc "NVIDIA Hardware Encoder")
  return()
endif()

if(NOT TARGET OBS::opts-parser)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/opts-parser" "${CMAKE_BINARY_DIR}/shared/opts-parser")
endif()

if(OS_LINUX AND NOT TARGET OBS::glad)
  add_subdirectory("${CMAKE_SOURCE_DIR}/deps/glad" "${CMAKE_BINARY_DIR}/deps/glad")
endif()

find_package(FFnvcodec 12 REQUIRED)

add_library(obs-nvenc MODULE)
add_library(OBS::nvenc ALIAS obs-nvenc)

include(cmake/sources.cmake)
obs_nvenc_sources(_obs_nvenc_common common)
obs_nvenc_sources(_obs_nvenc_linux linux)
obs_nvenc_sources(_obs_nvenc_windows windows)
obs_nvenc_sources(_obs_nvenc_headers headers)
obs_nvenc_sources(_obs_nvenc_reference reference)
obs_nvenc_sources(_obs_nvenc_rust rust)
obs_nvenc_sources(_obs_nvenc_helper helper)
add_subdirectory(obs-nvenc-test)
set(
  _obs_nvenc_sources
  "$<$<PLATFORM_ID:Linux>:${_obs_nvenc_linux}>"
  "$<$<PLATFORM_ID:Windows>:${_obs_nvenc_windows}>"
  ${_obs_nvenc_common}
  ${_obs_nvenc_headers}
)

# Production uses the completed Rust CUDA loader. Its C baseline is explicit,
# excluded from normal builds and never installed, like libobs-c-reference.
set(_obs_nvenc_targets obs-nvenc)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-nvenc
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_sources(obs-nvenc PRIVATE ${_obs_nvenc_sources})
if(OBS_LIBOBS_BACKEND STREQUAL HYBRID)
  target_sources(obs-nvenc PRIVATE ${_obs_nvenc_rust})
  corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
    CRATES obs-nvenc-plugin
    CRATE_TYPES staticlib
    PROFILE release
  )
  obs_serialize_cargo_build(obs-nvenc-plugin)
  target_link_libraries(obs-nvenc PRIVATE obs_nvenc_plugin-static)
  if(OS_LINUX)
    target_link_options(obs-nvenc PRIVATE "LINKER:--exclude-libs,ALL")
  endif()
  if(OBS_BUILD_C_REFERENCE)
    add_library(obs-nvenc-c-reference MODULE EXCLUDE_FROM_ALL ${_obs_nvenc_sources} ${_obs_nvenc_reference})
    list(APPEND _obs_nvenc_targets obs-nvenc-c-reference)
    set_target_properties(
      obs-nvenc-c-reference
      PROPERTIES
        FOLDER plugins/obs-nvenc
        PREFIX ""
        OUTPUT_NAME obs-nvenc
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/obs-nvenc-c-reference"
        ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/obs-nvenc-c-reference"
    )
  endif()
else()
  target_sources(obs-nvenc PRIVATE ${_obs_nvenc_reference})
endif()

foreach(target IN LISTS _obs_nvenc_targets)
  target_link_libraries(
    ${target}
    PRIVATE OBS::libobs OBS::opts-parser FFnvcodec::FFnvcodec $<$<PLATFORM_ID:Linux>:OBS::glad>
  )
  target_compile_definitions(
    ${target}
    PRIVATE
      $<$<BOOL:${ENABLE_NVENC_FFMPEG_IDS}>:REGISTER_FFMPEG_IDS>
      $<$<BOOL:${OBS_NVENC_TEST_NAME}>:OBS_NVENC_TEST_NAME="${OBS_NVENC_TEST_NAME}">
  )
endforeach()

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-nvenc.rc)
  target_sources(obs-nvenc PRIVATE obs-nvenc.rc)
endif()

set_target_properties_obs(obs-nvenc PROPERTIES FOLDER plugins/obs-nvenc PREFIX "")
