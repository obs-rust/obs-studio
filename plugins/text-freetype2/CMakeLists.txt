cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_FREETYPE "Enable FreeType text plugin" ON)

if(NOT ENABLE_FREETYPE)
  return()
endif()

find_package(Freetype REQUIRED)

if(OS_MACOS)
  find_package(Iconv REQUIRED)
elseif(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(Fontconfig REQUIRED)
endif()

add_library(text-freetype2 MODULE)
add_library(OBS::text-freetype2 ALIAS text-freetype2)

# Unity groups (BUILD.md), listed plainly per platform: groups.cmake doesn't
# read sources behind generator expressions.
target_sources(
  text-freetype2
  PRIVATE
    build/text-freetype2.core+.h.c
    find-font.h
    glyph-atlas.h
    obs-convenience.h
    text-file.h
    text-freetype2-host.h
    text-freetype2.h
)
if(OS_WINDOWS)
  target_sources(text-freetype2 PRIVATE build/text-freetype2.find-font+.h.c build/text-freetype2.windows+.h.c)
elseif(OS_MACOS)
  target_sources(
    text-freetype2
    PRIVATE build/text-freetype2.find-font+.h.c build/text-freetype2.macos+.h.c build/text-freetype2.macos-objc+.h.m
  )
else()
  target_sources(text-freetype2 PRIVATE build/text-freetype2.nix+.h.c)
endif()

target_link_libraries(
  text-freetype2
  PRIVATE
    OBS::libobs
    Freetype::Freetype
    $<$<PLATFORM_ID:Darwin>:Iconv::Iconv>
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>>"
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Fontconfig::Fontconfig>
)

# FreeType is hard-coded with /DEFAULTLIB:MSVCRT
target_link_options(text-freetype2 PRIVATE $<$<PLATFORM_ID:Windows>:/IGNORE:4098>)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in text-freetype2.rc)
  target_sources(text-freetype2 PRIVATE text-freetype2.rc)
endif()

set_target_properties_obs(text-freetype2 PROPERTIES FOLDER plugins PREFIX "")

# #2625: this plugin's C sources have their conversions fixed at the call sites,
# so the pins that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
target_compile_options(
  text-freetype2
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
