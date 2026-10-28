cmake_minimum_required(VERSION 3.28...3.30)

if(NOT ENABLE_PULSEAUDIO)
  return()
endif()

find_package(PulseAudio REQUIRED)

add_library(linux-pulseaudio MODULE)
add_library(OBS::pulseaudio ALIAS linux-pulseaudio)

target_sources(linux-pulseaudio PRIVATE build/linux-pulseaudio.nix+.h.c)
target_link_libraries(linux-pulseaudio PRIVATE OBS::libobs PulseAudio::PulseAudio)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  linux-pulseaudio
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

set_target_properties_obs(linux-pulseaudio PROPERTIES FOLDER plugins PREFIX "")
