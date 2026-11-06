cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_OSS "Enable building with OSS audio support" ON)

if(NOT ENABLE_OSS)
  return()
endif()

find_package(OSS REQUIRED)

add_library(oss-audio MODULE)
add_library(OBS::oss-audio ALIAS oss-audio)

# Unity groups (BUILD.md), listed plainly per platform: groups.cmake doesn't
# read sources behind generator expressions.
target_sources(oss-audio PRIVATE build/oss-audio.core+.h.c oss-input-platform.h)
if(CMAKE_SYSTEM_NAME MATCHES "^(FreeBSD|DragonFly)$")
  target_sources(oss-audio PRIVATE build/oss-audio.freebsd+.h.c)
else()
  target_sources(oss-audio PRIVATE build/oss-audio.linux+.h.c)
endif()

target_link_libraries(oss-audio PRIVATE OBS::libobs OSS::OSS)

# #2625: keep the native OSS implementation under the conversion ratchet.
target_compile_options(
  oss-audio
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

set_target_properties_obs(oss-audio PROPERTIES FOLDER plugins PREFIX "")
