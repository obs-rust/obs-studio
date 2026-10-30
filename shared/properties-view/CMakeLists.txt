cmake_minimum_required(VERSION 3.28...3.30)

find_package(Qt6 REQUIRED Core Widgets)

if(NOT TARGET OBS::qt-wrappers)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/wrappers" "${CMAKE_BINARY_DIR}/shared/qt/wrappers")
endif()

if(NOT TARGET OBS::qt-plain-text-edit)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/plain-text-edit" "${CMAKE_BINARY_DIR}/shared/qt/plain-text-edit")
endif()

if(NOT TARGET OBS::qt-vertical-scroll-area)
  add_subdirectory(
    "${CMAKE_SOURCE_DIR}/shared/qt/vertical-scroll-area"
    "${CMAKE_BINARY_DIR}/shared/qt/vertical-scroll-area"
  )
endif()

if(NOT TARGET OBS::qt-slider-ignorewheel)
  add_subdirectory(
    "${CMAKE_SOURCE_DIR}/shared/qt/slider-ignorewheel"
    "${CMAKE_BINARY_DIR}/shared/qt/slider-ignorewheel"
  )
endif()

if(NOT TARGET OBS::qt-icon-label)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/icon-label" "${CMAKE_BINARY_DIR}/shared/qt/icon-label")
endif()

if(NOT TARGET OBS::qt-obs-spinbox)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/obs-spinbox" "${CMAKE_BINARY_DIR}/shared/qt/obs-spinbox")
endif()

add_library(properties-view INTERFACE)
add_library(OBS::properties-view ALIAS properties-view)

target_sources(
  properties-view
  INTERFACE
    build/properties-view.core+.hpp.cpp
    double-slider.hpp
    properties-view.hpp
    properties-view.moc.hpp
    spinbox-ignorewheel.hpp
)
target_include_directories(properties-view INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")

target_link_libraries(
  properties-view
  INTERFACE
    OBS::frontend-api
    OBS::libobs
    OBS::qt-wrappers
    OBS::qt-plain-text-edit
    OBS::qt-obs-spinbox
    OBS::qt-vertical-scroll-area
    OBS::qt-slider-ignorewheel
    OBS::qt-icon-label
    Qt::Core
    Qt::Widgets
)

# The comparison helpers in properties-view.moc.hpp are intentionally static;
# MSVC may not need every helper in each interface consumer.
target_compile_options(properties-view INTERFACE "$<$<CXX_COMPILER_ID:MSVC>:/wd4505>")

# #2625: this library's C++ sources state their conversions at the call sites,
# so the pins that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
#
# Apple keeps the sign-conversion pin (#3096): these flags are INTERFACE, so
# they reach every consumer's TUs, and with the group on the macOS app lane
# stops inside Qt 6.11's own headers (qversiontagging.h, qnamespace.h), which
# no obs source states. The two platforms are stated as branches because the
# Xcode generator does not apply the Darwin half of a
# `$<NOT:$<PLATFORM_ID:Darwin>>` genex in `target_compile_options` (#2979).
if(OS_MACOS)
  target_compile_options(properties-view INTERFACE -Wno-sign-conversion)
else()
  target_compile_options(
    properties-view
    INTERFACE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
  )
endif()
