cmake_minimum_required(VERSION 3.28...3.30)

if(OS_WINDOWS)
  option(ENABLE_COREAUDIO_ENCODER "Enable building with CoreAudio encoder (Windows)" ON)
  if(NOT ENABLE_COREAUDIO_ENCODER)
    return()
  endif()
endif()

add_library(coreaudio-encoder MODULE)
add_library(OBS::coreaudio-encoder ALIAS coreaudio-encoder)
obs_msvc_exempt_c4201(coreaudio-encoder)

# SDK types and deleters belong to one host; the Windows imports/functions
# must share a translation unit with the encoder that calls them.
target_include_directories(
  coreaudio-encoder
  PRIVATE
    "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/macos>"
)

target_sources(
  coreaudio-encoder
  PRIVATE
    $<$<PLATFORM_ID:Windows>:build/coreaudio-encoder.windows+.hpp.cpp>
    $<$<PLATFORM_ID:Darwin>:build/coreaudio-encoder.macos+.hpp.cpp>
    $<$<PLATFORM_ID:Windows>:windows-imports.h>
)

target_link_libraries(
  coreaudio-encoder
  PRIVATE
    OBS::libobs
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,AudioToolbox.framework>>"
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,CoreAudio.framework>>"
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>>"
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in coreaudio-encoder.rc)
  target_sources(coreaudio-encoder PRIVATE coreaudio-encoder.rc)
endif()

set_target_properties_obs(coreaudio-encoder PROPERTIES FOLDER plugins PREFIX "")
