cmake_minimum_required(VERSION 3.28...3.30)

add_library(bpm OBJECT)
add_library(OBS::bpm ALIAS bpm)

target_sources(bpm PRIVATE build/bpm.core+.h.c bpm-internal.h PUBLIC bpm.h)

target_include_directories(bpm PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")

target_link_libraries(bpm PUBLIC OBS::libobs OBS::caption)

set_target_properties(bpm PROPERTIES FOLDER deps POSITION_INDEPENDENT_CODE TRUE)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  bpm
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
