cmake_minimum_required(VERSION 3.28...3.30)

add_library(obs-frontend-api SHARED)
add_library(OBS::frontend-api ALIAS obs-frontend-api)

target_sources(
  obs-frontend-api
  PRIVATE build/obs-frontend-api.core+.hpp.cpp obs-frontend-api.h obs-frontend-internal.hpp
)

target_link_libraries(obs-frontend-api PRIVATE OBS::libobs)

# #2625: this library's C++ source states its conversions at the call sites,
# so the pins that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
target_compile_options(
  obs-frontend-api
  PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_include_directories(obs-frontend-api PUBLIC "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>")

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-frontend-api.rc)
  target_sources(obs-frontend-api PRIVATE obs-frontend-api.rc)
elseif(OS_MACOS)
  set_target_properties(obs-frontend-api PROPERTIES SOVERSION 1)
endif()

set_target_properties_obs(obs-frontend-api PROPERTIES FOLDER frontend PUBLIC_HEADER obs-frontend-api.h)

if(OS_WINDOWS OR OS_MACOS)
  set_property(TARGET obs-frontend-api PROPERTY PREFIX "")
endif()

target_export(obs-frontend-api)
