cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_PIPEWIRE "Enable PipeWire support" ON)
if(NOT ENABLE_PIPEWIRE)
  return()
endif()

find_package(PipeWire 0.3.33 REQUIRED)
find_package(Gio 2.76 REQUIRED)
find_package(Libdrm REQUIRED)
get_target_property(libdrm_include_directories Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)

add_library(linux-pipewire MODULE)
add_library(OBS::pipewire ALIAS linux-pipewire)

if(PipeWire_VERSION VERSION_GREATER_EQUAL 0.3.60)
  target_enable_feature(linux-pipewire "PipeWire 0.3.60+ camera support")
  set(_HAS_PIPEWIRE_CAMERA TRUE)
else()
  set(_HAS_PIPEWIRE_CAMERA FALSE)
endif()

target_sources(
  linux-pipewire
  PRIVATE
    $<$<BOOL:${_HAS_PIPEWIRE_CAMERA}>:camera-portal.h>
    build/linux-pipewire.nix+.h.c
    formats.h
    frame-layout.h
    frame-pacing.h
    pipewire.h
    portal.h
    reconnect-policy.h
    screencast-portal.h
)
# The camera portal's unity group (BUILD.md), listed plainly: groups.cmake
# doesn't read sources behind generator expressions.
if(_HAS_PIPEWIRE_CAMERA)
  target_sources(linux-pipewire PRIVATE build/linux-pipewire.nix-camera+.h.c)
endif()

target_include_directories(linux-pipewire PRIVATE ${libdrm_include_directories})

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  linux-pipewire
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

target_link_libraries(linux-pipewire PRIVATE OBS::libobs OBS::glad PipeWire::PipeWire gio::gio)

set_target_properties_obs(linux-pipewire PROPERTIES FOLDER plugins PREFIX "")
