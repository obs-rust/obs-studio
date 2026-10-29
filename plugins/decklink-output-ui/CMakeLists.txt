cmake_minimum_required(VERSION 3.28...3.30)

if(NOT ENABLE_DECKLINK)
  return()
endif()

find_package(Qt6 REQUIRED Widgets)

if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(X11 REQUIRED)
endif()

add_library(decklink-output-ui MODULE)
add_library(OBS::decklink-output-ui ALIAS decklink-output-ui)
obs_msvc_exempt_c4201(decklink-output-ui)

if(NOT TARGET OBS::properties-view)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/properties-view" "${CMAKE_BINARY_DIR}/shared/properties-view")
endif()

target_sources(decklink-output-ui PRIVATE forms/output.ui)

target_sources(decklink-output-ui PRIVATE build/decklink-output-ui.core+.hpp.cpp DecklinkOutputUI.h decklink-ui-main.h)

target_compile_options(decklink-output-ui PRIVATE $<$<PLATFORM_ID:Darwin>:-Wno-quoted-include-in-framework-header>)

target_link_libraries(
  decklink-output-ui
  PRIVATE
    OBS::libobs
    OBS::frontend-api
    OBS::properties-view
    Qt::Widgets
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>>"
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:X11::X11>
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in decklink-output-ui.rc)
  target_sources(decklink-output-ui PRIVATE decklink-output-ui.rc)

  set_property(TARGET decklink-output-ui APPEND PROPERTY AUTORCC_OPTIONS --format-version 1)
endif()

set_target_properties_obs(
  decklink-output-ui
  PROPERTIES FOLDER plugins/decklink
             PREFIX ""
             AUTOMOC ON
             AUTOUIC ON
             AUTORCC ON
             AUTOUIC_SEARCH_PATHS forms
)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  decklink-output-ui
  PRIVATE
    "$<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
    "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
