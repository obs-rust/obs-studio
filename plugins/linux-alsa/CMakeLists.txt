cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_ALSA "Build OBS with ALSA support" ON)

if(NOT ENABLE_ALSA)
  return()
endif()

find_package(ALSA REQUIRED)

add_library(linux-alsa MODULE)
add_library(OBS::alsa ALIAS linux-alsa)

target_sources(linux-alsa PRIVATE build/linux-alsa.nix+.h.c)
target_link_libraries(linux-alsa PRIVATE OBS::libobs ALSA::ALSA)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  linux-alsa
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

set_target_properties_obs(linux-alsa PROPERTIES FOLDER plugins PREFIX "")
