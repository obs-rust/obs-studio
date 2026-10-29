cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_VST "Enable building OBS with VST plugin" ON)

if(NOT ENABLE_VST)
  return()
endif()

add_library(obs-vst MODULE)
add_library(OBS::vst ALIAS obs-vst)
obs_msvc_exempt_c4201(obs-vst)

find_package(Qt6 REQUIRED Widgets)

target_sources(
  obs-vst
  PRIVATE
    $<$<PLATFORM_ID:Darwin>:build/obs-vst.macos+.hpp.mm>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-vst.linux+.hpp.cpp>
    $<$<PLATFORM_ID:Windows>:build/obs-vst.windows+.hpp.cpp>
    build/obs-vst.core+.hpp.cpp
    headers/EditorWidget.h
    headers/vst-plugin-callbacks.hpp
    headers/VSTPlugin.h
    vst_header/aeffectx.h
)

target_include_directories(
  obs-vst
  PRIVATE
    vst_header
    "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/macos>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/linux>"
)

target_link_libraries(
  obs-vst
  PRIVATE
    OBS::libobs
    Qt::Widgets
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>>"
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Foundation.framework>>"
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-vst.rc)
  target_sources(obs-vst PRIVATE obs-vst.rc)
  set_property(TARGET obs-vst APPEND PROPERTY AUTORCC_OPTIONS --format-version 1)
endif()

set_target_properties_obs(
  obs-vst
  PROPERTIES FOLDER plugins
             PREFIX ""
             AUTOMOC ON
             AUTOUIC ON
             AUTORCC ON
)

# #2625: this plugin's C++ sources name no implicit conversion with the three
# groups on, so the pins that hide them are not needed here. The global
# -Wno-conversion / -Wno-sign-conversion / -Wno-float-conversion only leave the
# flag lists once every target is ratcheted.
target_compile_options(
  obs-vst
  PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
