cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_LIBFDK "Enable FDK AAC support" OFF)
if(NOT ENABLE_LIBFDK)
  return()
endif()

find_package(Libfdk REQUIRED)

add_library(obs-libfdk MODULE)
add_library(OBS::libfdk ALIAS obs-libfdk)

target_sources(obs-libfdk PRIVATE build/obs-libfdk.core+.h.c)

# #2625: dependency-gated target, so no PR lane compiles it. Opted into the
# conversion ratchet; the global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is
# ratcheted.
target_compile_options(
  obs-libfdk
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_link_libraries(obs-libfdk PRIVATE OBS::libobs Libfdk::Libfdk)

set_target_properties_obs(obs-libfdk PROPERTIES FOLDER plugins PREFIX "")
