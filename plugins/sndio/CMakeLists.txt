cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_SNDIO "Build OBS with sndio support" OFF)
if(NOT ENABLE_SNDIO)
  return()
endif()

find_package(Sndio REQUIRED)

add_library(sndio MODULE)
add_library(OBS::sndio ALIAS sndio)

target_sources(sndio PRIVATE build/sndio.nix+.h.c)

target_compile_options(sndio PRIVATE -Wno-error=vla)

# #2625: dependency-gated target (ENABLE_SNDIO is off in every CI lane), so no
# lane has compiled it with the conversion groups. Opted into the ratchet; the
# global -Wno-conversion / -Wno-sign-conversion / -Wno-float-conversion only
# leave the flag lists once every target is ratcheted.
target_compile_options(
  sndio
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_link_libraries(sndio PRIVATE OBS::libobs Sndio::Sndio)

set_target_properties_obs(sndio PROPERTIES FOLDER plugins PREFIX "")
