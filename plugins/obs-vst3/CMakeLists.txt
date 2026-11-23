cmake_minimum_required(VERSION 3.28...3.30)

if(NOT ENABLE_VST3)
  return()
endif()

find_package(Qt6 REQUIRED Widgets)
include(cmake/sdk.cmake)

add_library(
  obs-vst3-discovery
  STATIC
  build/obs-vst3.discovery+.hpp.cpp
  "build/obs-vst3.discovery.${_vst3_host}+.hpp.cpp"
)
target_link_libraries(obs-vst3-discovery PUBLIC OBS::libobs OBS::vst3-sdk)
set_target_properties(obs-vst3-discovery PROPERTIES FOLDER plugins/obs-vst3 POSITION_INDEPENDENT_CODE ON)

add_subdirectory(vst3-scan)

add_library(obs-vst3 MODULE)
add_library(OBS::vst3 ALIAS obs-vst3)
add_dependencies(obs-vst3 obs-vst3-scanner)

target_sources(
  obs-vst3
  PRIVATE
    VST3Plugin.h
    build/obs-vst3.core+.hpp.cpp
    build/obs-vst3.editor+.hpp.cpp
    "$<$<PLATFORM_ID:Linux>:build/obs-vst3.editor.linux+.hpp.cpp>"
    "$<$<PLATFORM_ID:Windows>:build/obs-vst3.editor.windows+.hpp.cpp>"
    "$<$<PLATFORM_ID:Linux>:build/obs-vst3.linux+.hpp.cpp>"
    "$<$<PLATFORM_ID:Darwin>:build/obs-vst3.macos+.hpp.mm>"
    "$<$<PLATFORM_ID:Darwin>:build/obs-vst3.host.macos+.hpp.cpp>"
    "$<$<PLATFORM_ID:Windows>:build/obs-vst3.host.windows+.hpp.cpp>"
)
target_compile_options(obs-vst3 PRIVATE "$<$<COMPILE_LANG_AND_ID:OBJCXX,AppleClang>:-fobjc-arc>")
if(OS_MACOS)
  set_target_properties(obs-vst3 PROPERTIES XCODE_ATTRIBUTE_CLANG_ENABLE_OBJC_ARC YES)
endif()
target_link_libraries(obs-vst3 PRIVATE OBS::frontend-api obs-vst3-discovery Qt::Widgets)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-vst3.rc)
  configure_file(cmake/windows/obs-studio.ico obs-studio.ico COPYONLY)
  target_sources(obs-vst3 PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/obs-vst3.rc")
  obs_msvc_exempt_c4201(obs-vst3)
endif()

# VS forbids a project and its solution folder from sharing the same name.
set_target_properties_obs(obs-vst3 PROPERTIES FOLDER plugins/obs-vst3 PREFIX "" AUTOMOC ON)
