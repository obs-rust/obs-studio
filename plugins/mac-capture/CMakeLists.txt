cmake_minimum_required(VERSION 3.28...3.30)

add_library(mac-capture MODULE)
add_library(OBS::capture ALIAS mac-capture)

include(sources.cmake)
obs_macos_capture_sources(_capture_sources MAC_CAPTURE "${CMAKE_CURRENT_SOURCE_DIR}")

target_sources(
  mac-capture
  PRIVATE
    ${_capture_sources}
    audio-device-enum.h
    coreaudio-retry.h
    mac-sck-common.h
    mac-sck-stall.h
    mac-sck-stop.h
    window-utils.h
)

target_link_libraries(
  mac-capture
  PRIVATE
    OBS::libobs
    "$<LINK_LIBRARY:FRAMEWORK,AudioUnit.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreAudio.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreMedia.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreVideo.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,IOSurface.framework>"
    "$<LINK_LIBRARY:WEAK_FRAMEWORK,ScreenCaptureKit.framework>"
)

set_target_properties_obs(
  mac-capture
  PROPERTIES FOLDER plugins
             PREFIX ""
             XCODE_ATTRIBUTE_CLANG_WARN_SUSPICIOUS_IMPLICIT_CONVERSION YES
             XCODE_ATTRIBUTE_GCC_WARN_SHADOW YES
)

if(CMAKE_VERSION VERSION_LESS_EQUAL 3.25.0)
  set_property(TARGET mac-capture PROPERTY XCODE_LINK_BUILD_PHASE_MODE BUILT_ONLY)
endif()
