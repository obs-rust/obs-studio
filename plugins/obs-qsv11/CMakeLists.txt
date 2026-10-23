cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_QSV11 "Build Intel QSV11 Hardware Encoder." TRUE)
if(NOT ENABLE_QSV11)
  target_disable_feature(obs-qsv11 "Intel QSV11 Hardware Encoder")
  return()
endif()

add_library(obs-qsv11 MODULE)
add_library(OBS::qsv11 ALIAS obs-qsv11)

find_package(VPL 2.9 REQUIRED)

if(OS_LINUX)
  find_package(Libva REQUIRED)
endif()

target_sources(
  obs-qsv11
  PRIVATE
    $<$<PLATFORM_ID:Linux>:build/obs-qsv11.linux+.hpp.cpp>
    $<$<PLATFORM_ID:Linux>:build/obs-qsv11.linux-c+.h.c>
    $<$<PLATFORM_ID:Windows>:build/obs-qsv11.windows+.hpp.cpp>
    $<$<PLATFORM_ID:Windows>:build/obs-qsv11.windows-c+.h.c>
    $<$<PLATFORM_ID:Windows>:common_directx11.h>
    build/obs-qsv11.core+.hpp.cpp
    build/obs-qsv11.c+.h.c
    build/obs-qsv11.probe+.h.c
    common_utils.h
    qsv-host.h
    qsv-probe.h
    QSV_Encoder.h
    QSV_Encoder_Internal.h
)

# The host's bits/<os>_defs.h for common_utils.h (#860).
target_compile_definitions(
  obs-qsv11
  PRIVATE
    $<$<PLATFORM_ID:Linux>:OBS_QSV_HOST_DEFS="plugins/obs-qsv11/bits/linux_defs.h">
    $<$<PLATFORM_ID:Windows>:OBS_QSV_HOST_DEFS="plugins/obs-qsv11/bits/windows_defs.h">
)

target_compile_options(
  obs-qsv11
  PRIVATE
    $<$<COMPILE_LANG_AND_ID:C,GNU,Clang>:-Wno-error=switch>
    $<$<COMPILE_LANG_AND_ID:CXX,GNU,Clang>:-Wno-error=switch>
)

target_link_libraries(
  obs-qsv11
  PRIVATE
    OBS::libobs
    VPL::VPL
    $<$<PLATFORM_ID:Linux>:Libva::va>
    $<$<PLATFORM_ID:Linux>:Libva::drm>
    $<$<PLATFORM_ID:Windows>:d3d11>
    $<$<PLATFORM_ID:Windows>:dxgi>
    $<$<PLATFORM_ID:Windows>:dxguid>
)

target_link_options(obs-qsv11 PRIVATE $<$<PLATFORM_ID:Windows>:/IGNORE:4099>)

if(OS_WINDOWS)
  add_subdirectory(obs-qsv-test)

  configure_file(cmake/windows/obs-module.rc.in obs-qsv11.rc)
  target_sources(obs-qsv11 PRIVATE obs-qsv11.rc)
endif()

set_target_properties_obs(obs-qsv11 PROPERTIES FOLDER plugins/obs-qsv11 PREFIX "")
