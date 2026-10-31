cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_JACK "Build OBS with JACK support" OFF)
if(NOT ENABLE_JACK)
  return()
endif()

find_package(Jack REQUIRED)

add_library(linux-jack MODULE)
add_library(OBS::jack ALIAS linux-jack)

target_sources(linux-jack PRIVATE build/linux-jack.nix+.h.c)

target_compile_options(linux-jack PRIVATE $<$<COMPILE_LANG_AND_ID:C,Clang>:-Wno-error=sign-compare>)

# #2625: this target reports no conversions under the three groups, so the pins
# that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
target_compile_options(
  linux-jack
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_link_libraries(linux-jack PRIVATE OBS::libobs Jack::Jack)

set_target_properties_obs(linux-jack PROPERTIES FOLDER plugins PREFIX "" PROJECT_LABEL "JACK Audio")
