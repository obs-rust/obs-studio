cmake_minimum_required(VERSION 3.28...3.30)

find_package(Detours REQUIRED)
find_package(Vulkan REQUIRED)

if(NOT TARGET OBS::ipc-util)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/ipc-util" ipc-util)
endif()

if(NOT TARGET OBS::obfuscate)
  add_subdirectory("${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/libobs")
endif()

if(NOT TARGET OBS::d3d8-api)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/obs-d3d8-api" obs-d3d8-api)
endif()

if(NOT TARGET OBS::hook-config)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/obs-hook-config" obs-hook-config)
endif()

add_library(graphics-hook MODULE)
# Its sources each use a different Direct3D version (d3d8.h, d3d9.h, d3d10,
# d3d11, d3d12), whose headers can't share a translation unit: no unity
# batches (cmake/common/unity.cmake).
set_property(TARGET graphics-hook PROPERTY OBS_DISABLE_UNITY_BUILD ON)
add_library(OBS::graphics-hook ALIAS graphics-hook)
# MSVC /W4 (#2626): this target is also configured by the child architecture
# build (cmake/windows/architecture.cmake), which does not include
# cmake/common/helpers_common.cmake, so the C4201 exemption is written inline
# here instead of through the per-target helper. d3d10-capture.cpp.hpp's state
# is an anonymous union over two struct overlays by design.
target_compile_options(graphics-hook PRIVATE $<$<COMPILE_LANG_AND_ID:CXX,MSVC>:/wd4201>)

target_sources(
  graphics-hook
  PRIVATE
    build/graphics-hook.d3d10-capture-windows+.hpp.cpp
    build/graphics-hook.d3d11-capture-windows+.hpp.cpp
    build/graphics-hook.d3d12-capture-windows+.hpp.cpp
    build/graphics-hook.d3d8-capture-windows+.hpp.cpp
    build/graphics-hook.d3d9-capture-windows+.hpp.cpp
    d3d9-patches.hpp
    build/graphics-hook.dxgi-capture-windows+.hpp.cpp
    build/graphics-hook.gl-capture-windows+.h.c
    gl-decs.h
    build/graphics-hook.graphics-hook-windows+.h.c
    graphics-hook.h
    graphics-hook.rc
)

target_compile_definitions(graphics-hook PRIVATE COMPILE_D3D12_HOOK)

target_link_libraries(
  graphics-hook
  PRIVATE OBS::d3d8-api OBS::hook-config OBS::ipc-util OBS::obfuscate Detours::Detours dxguid
)

target_link_options(graphics-hook PRIVATE /IGNORE:4099)

if(TARGET Vulkan::Vulkan)
  target_sources(graphics-hook PRIVATE build/graphics-hook.vulkan-capture-windows+.h.c vulkan-capture.h)
  target_link_libraries(graphics-hook PRIVATE Vulkan::Vulkan)
  target_compile_definitions(graphics-hook PRIVATE COMPILE_VULKAN_HOOK)
endif()

if(OBS_PARENT_ARCHITECTURE STREQUAL CMAKE_VS_PLATFORM_NAME)
  if(CMAKE_VS_PLATFORM_NAME STREQUAL ARM64)
    add_custom_command(
      TARGET graphics-hook
      POST_BUILD
      COMMAND "${CMAKE_COMMAND}" --build ${CMAKE_SOURCE_DIR}/build_x64 --config $<CONFIG> -t graphics-hook
      COMMENT "Build x64 graphics-hook"
    )
  endif()

  if(CMAKE_VS_PLATFORM_NAME MATCHES "(ARM64|x64)")
    add_custom_command(
      TARGET graphics-hook
      POST_BUILD
      COMMAND "${CMAKE_COMMAND}" --build ${CMAKE_SOURCE_DIR}/build_x86 --config $<CONFIG> -t graphics-hook
      COMMENT "Build x86 graphics-hook"
    )
    obs_serialize_x86_build(graphics-hook)
  endif()

  add_dependencies(win-capture graphics-hook)
endif()

if(CMAKE_VS_PLATFORM_NAME STREQUAL ARM64)
  set(_OUTPUT_NAME graphics-hook-arm64)
elseif(CMAKE_VS_PLATFORM_NAME STREQUAL x64)
  set(_OUTPUT_NAME graphics-hook64)
else()
  set(_OUTPUT_NAME graphics-hook32)
endif()

set_target_properties_obs(
  graphics-hook PROPERTIES
                FOLDER "plugins/win-capture"
                OUTPUT_NAME ${_OUTPUT_NAME}
                MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)
