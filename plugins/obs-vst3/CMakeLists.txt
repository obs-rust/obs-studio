cmake_minimum_required(VERSION 3.28...3.30)

if(NOT ENABLE_VST3 OR NOT OBS_LIBOBS_BACKEND STREQUAL HYBRID)
  return()
endif()

find_package(Qt6 REQUIRED Widgets)
include(cmake/sdk.cmake)

if(OBS_BUILD_C_REFERENCE OR ENABLE_UNIT_TESTS)
  add_library(
    obs-vst3-discovery
    STATIC
    EXCLUDE_FROM_ALL
    build/obs-vst3.discovery+.hpp.cpp
    "build/obs-vst3.discovery.${_vst3_host}+.hpp.cpp"
  )
  target_link_libraries(obs-vst3-discovery PUBLIC OBS::libobs OBS::vst3-sdk)
  set_target_properties(obs-vst3-discovery PROPERTIES FOLDER plugins/obs-vst3 POSITION_INDEPENDENT_CODE ON)
endif()

add_subdirectory(vst3-scan)

add_library(obs-vst3 MODULE)
add_library(OBS::vst3 ALIAS obs-vst3)
add_dependencies(obs-vst3 obs-vst3-scanner)

set(
  _obs_vst3_host_sources
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
target_sources(obs-vst3 PRIVATE ${_obs_vst3_host_sources})
target_compile_options(obs-vst3 PRIVATE "$<$<COMPILE_LANG_AND_ID:OBJCXX,AppleClang>:-fobjc-arc>")
if(OS_MACOS)
  set_target_properties(obs-vst3 PROPERTIES XCODE_ATTRIBUTE_CLANG_ENABLE_OBJC_ARC YES)
endif()
target_link_libraries(obs-vst3 PRIVATE OBS::frontend-api OBS::libobs OBS::vst3-sdk Qt::Widgets)
corrosion_import_crate(
  MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
  CRATES obs-vst3-native
  CRATE_TYPES staticlib
  PROFILE release
)
obs_serialize_cargo_build(obs-vst3)
target_compile_definitions(obs-vst3 PRIVATE OBS_VST3_RUST_HOST)
target_link_libraries(
  obs-vst3
  PRIVATE obs_vst3-static "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>>"
)
if(OS_LINUX)
  target_link_options(obs-vst3 PRIVATE "LINKER:--exclude-libs,ALL")
  if(ENABLE_UNIT_TESTS)
    add_library(obs-vst3-c-reference MODULE EXCLUDE_FROM_ALL ${_obs_vst3_host_sources})
    target_link_libraries(obs-vst3-c-reference PRIVATE OBS::frontend-api obs-vst3-discovery Qt::Widgets)
    set_target_properties(obs-vst3-c-reference PROPERTIES PREFIX "" AUTOMOC ON)
    if(OBS_VST3_SCANNER_NAME)
      target_compile_definitions(obs-vst3-c-reference PRIVATE OBS_VST3_SCANNER_NAME="${OBS_VST3_SCANNER_NAME}")
    endif()

    # Link the same host source set into test executables so the probe can
    # call the private C++ adapter without exporting it from the modules.
    foreach(variant IN ITEMS cpp rust)
      set(probe "test-vst3-variable-${variant}")
      add_executable(
        "${probe}"
        EXCLUDE_FROM_ALL
        ${_obs_vst3_host_sources}
        "${CMAKE_CURRENT_SOURCE_DIR}/../../test/cmocka/vst3-test-variable.cpp"
      )
      target_link_libraries("${probe}" PRIVATE OBS::frontend-api OBS::libobs OBS::vst3-sdk Qt::Widgets)
      set_target_properties("${probe}" PROPERTIES AUTOMOC ON)
    endforeach()
    target_compile_definitions(test-vst3-variable-rust PRIVATE OBS_VST3_RUST_HOST)
    target_link_libraries(test-vst3-variable-cpp PRIVATE obs-vst3-discovery)
    target_link_libraries(test-vst3-variable-rust PRIVATE obs_vst3-static)
  endif()
endif()
if(OBS_VST3_SCANNER_NAME)
  target_compile_definitions(obs-vst3 PRIVATE OBS_VST3_SCANNER_NAME="${OBS_VST3_SCANNER_NAME}")
endif()

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in obs-vst3.rc)
  configure_file(cmake/windows/obs-studio.ico obs-studio.ico COPYONLY)
  target_sources(obs-vst3 PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/obs-vst3.rc")
  obs_msvc_exempt_c4201(obs-vst3)
endif()

# VS forbids a project and its solution folder from sharing the same name.
set_target_properties_obs(obs-vst3 PROPERTIES FOLDER plugins/obs-vst3 PREFIX "" AUTOMOC ON)

if(OS_WINDOWS OR OS_MACOS)
  include("${CMAKE_SOURCE_DIR}/test/cmocka/vst3-package.cmake")
endif()
