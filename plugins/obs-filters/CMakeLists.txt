cmake_minimum_required(VERSION 3.28...3.30)

add_library(obs-filters MODULE)
add_library(OBS::filters ALIAS obs-filters)

target_sources(obs-filters PRIVATE build/obs-filters.core+.h.c cube-lut.h)

target_link_libraries(obs-filters PRIVATE OBS::libobs $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-filters
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

include(cmake/speexdsp.cmake)
include(cmake/rnnoise.cmake)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-filters.rc)
  target_sources(obs-filters PRIVATE obs-filters.rc)
endif()

set_target_properties_obs(obs-filters PROPERTIES FOLDER plugins PREFIX "")
