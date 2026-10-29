cmake_minimum_required(VERSION 3.28...3.30)

add_library(obs-transitions MODULE)
add_library(OBS::transition ALIAS obs-transitions)

target_sources(obs-transitions PRIVATE build/obs-transitions.core+.h.c)

target_link_libraries(obs-transitions PRIVATE OBS::libobs)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-transitions
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-transitions.rc)
  target_sources(obs-transitions PRIVATE obs-transitions.rc)
endif()

set_target_properties_obs(obs-transitions PROPERTIES FOLDER plugins PREFIX "")
