cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_WEBRTC "Enable WebRTC Output support" ON)
if(NOT ENABLE_WEBRTC)
  return()
endif()

find_package(LibDataChannel 0.20 REQUIRED)
find_package(CURL REQUIRED)

add_library(obs-webrtc MODULE)
add_library(OBS::webrtc ALIAS obs-webrtc)

target_sources(
  obs-webrtc
  PRIVATE
    $<$<PLATFORM_ID:Darwin>:build/obs-webrtc.macos+.hpp.cpp>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-webrtc.posix+.hpp.cpp>
    $<$<PLATFORM_ID:Windows>:build/obs-webrtc.windows+.hpp.cpp>
    build/obs-webrtc.core+.hpp.cpp
    whip-output.h
    whip-service.h
    whip-utils.h
    whip-host.h
    whip-rtp-timestamp.h
)

target_link_libraries(obs-webrtc PRIVATE OBS::libobs LibDataChannel::LibDataChannel CURL::libcurl)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-webrtc.rc)
  target_sources(obs-webrtc PRIVATE obs-webrtc.rc)
endif()

set_target_properties_obs(obs-webrtc PROPERTIES FOLDER plugins PREFIX "")

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-webrtc
  PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
