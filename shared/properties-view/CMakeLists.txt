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

# The sources are compiled once, by a target of their own, and the interface
# target carries only what a consumer needs to include and link them. They used to
# be INTERFACE sources with the conversion groups as INTERFACE options, which made
# every consumer compile its own copy *and* take the groups; CMake appends a linked
# target's INTERFACE options after the consumer's own, so a consumer still holding
# the global pins could not pin them back off (#3096 on Apple, #3164 on Qt 6.8's
# `QString::arg(double)` in the frontend). A group is a statement about the sources
# that were converted, so it is PRIVATE on the target that compiles them.
add_library(properties-view INTERFACE)
add_library(OBS::properties-view ALIAS properties-view)

add_library(properties-view-code OBJECT build/properties-view.core+.hpp.cpp)
set_target_properties(properties-view-code PROPERTIES FOLDER shared)

target_sources(
  properties-view
  INTERFACE double-slider.hpp properties-view.hpp properties-view.moc.hpp spinbox-ignorewheel.hpp
)
target_include_directories(properties-view INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")

# The interface plain sources (headers) stay on the INTERFACE target: Qt's AUTOMOC
# scans the consumer's listed files for Q_OBJECT, and those metas are generated
# once per binary, not once for the interface.
target_link_libraries(
  properties-view
  INTERFACE
    properties-view-code
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

# The object library needs the same usage requirements to compile the TU; PRIVATE
# so that nothing it states reaches the binaries that link the objects. The Qt
# helper libraries are INTERFACE targets that carry their own unity sources, so
# linking them here would compile those TUs a second time and the consumer that
# compiles them itself would define the same symbols: the object target takes the
# headers and definitions they publish, not their sources.
set(
  _properties_view_header_deps
  OBS::qt-wrappers
  OBS::qt-plain-text-edit
  OBS::qt-obs-spinbox
  OBS::qt-vertical-scroll-area
  OBS::qt-slider-ignorewheel
  OBS::qt-icon-label
)
foreach(_properties_view_dep IN LISTS _properties_view_header_deps)
  get_target_property(_properties_view_includes ${_properties_view_dep} INTERFACE_INCLUDE_DIRECTORIES)
  if(_properties_view_includes)
    target_include_directories(properties-view-code PRIVATE ${_properties_view_includes})
  endif()
  get_target_property(_properties_view_definitions ${_properties_view_dep} INTERFACE_COMPILE_DEFINITIONS)
  if(_properties_view_definitions)
    target_compile_definitions(properties-view-code PRIVATE ${_properties_view_definitions})
  endif()
endforeach()

target_link_libraries(properties-view-code PRIVATE OBS::frontend-api OBS::libobs Qt::Core Qt::Widgets)

# The comparison helpers in properties-view.moc.hpp are intentionally static;
# MSVC may not need every helper in each interface consumer.
target_compile_options(properties-view INTERFACE "$<$<CXX_COMPILER_ID:MSVC>:/wd4505>")
target_compile_options(properties-view-code PRIVATE "$<$<CXX_COMPILER_ID:MSVC>:/wd4505>")

# #2625: this library's C++ sources state their conversions at the call sites,
# so the pins that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
#
# The groups are PRIVATE on `properties-view-code`, the target that compiles the
# unity TU. Stating them INTERFACE handed them to every consumer's TUs as well,
# after the consumer's own options, so the frontend's global pins never applied
# (#3164). Apple keeps the sign-conversion pin (#3096) rather than the group:
# with it on, the macOS app lane stops inside Qt 6.11's own headers
# (qversiontagging.h, qnamespace.h), which no obs source states.
if(OS_MACOS)
  target_compile_options(properties-view-code PRIVATE -Wno-sign-conversion)
else()
  target_compile_options(
    properties-view-code
    PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
  )
endif()
