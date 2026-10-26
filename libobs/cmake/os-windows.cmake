if(NOT DEFINED LIBOBS_WINDOWS_OBFUSCATE_SOURCES)
  include("${CMAKE_CURRENT_LIST_DIR}/sources.cmake")
endif()

if(NOT TARGET OBS::obfuscate)
  add_library(obs-obfuscate INTERFACE)
  add_library(OBS::obfuscate ALIAS obs-obfuscate)
  target_sources(obs-obfuscate INTERFACE ${LIBOBS_WINDOWS_OBFUSCATE_SOURCES} util/windows/obfuscate.h)
  target_include_directories(obs-obfuscate INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
endif()

if(NOT TARGET OBS::comutils)
  add_library(obs-comutils INTERFACE)
  add_library(OBS::COMutils ALIAS obs-comutils)
  target_sources(obs-comutils INTERFACE util/windows/ComPtr.hpp)
  target_include_directories(obs-comutils INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
endif()

if(NOT TARGET OBS::winhandle)
  add_library(obs-winhandle INTERFACE)
  add_library(OBS::winhandle ALIAS obs-winhandle)
  target_sources(obs-winhandle INTERFACE util/windows/WinHandle.hpp)
  target_include_directories(obs-winhandle INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
endif()

if(NOT TARGET OBS::threading-windows)
  add_library(obs-threading-windows INTERFACE)
  add_library(OBS::threading-windows ALIAS obs-threading-windows)
  target_sources(obs-threading-windows INTERFACE util/threading-windows.h)
  target_include_directories(obs-threading-windows INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
endif()

if(NOT TARGET OBS::w32-pthreads)
  add_subdirectory("${CMAKE_SOURCE_DIR}/deps/w32-pthreads" "${CMAKE_BINARY_DIR}/deps/w32-pthreads")
endif()

if(NOT OBS_PARENT_ARCHITECTURE STREQUAL CMAKE_VS_PLATFORM_NAME)
  return()
endif()

configure_file(cmake/windows/obs-module.rc.in libobs.rc)

target_sources(libobs-objects PRIVATE libobs.rc)
libobs_add_host_sources(WINDOWS)

target_compile_options(libobs-objects PUBLIC $<$<COMPILE_LANGUAGE:C,CXX>:/EHc->)

set_source_files_properties(
  build/libobs.windows-crash-handler+.h.c
  PROPERTIES COMPILE_DEFINITIONS OBS_VERSION="${OBS_VERSION_CANONICAL}"
)

set_source_files_properties(
  build/libobs.nsgif+.h.c
  build/libobs.ported-nsgif-blocks+.h.c
  build/libobs.ported-nsgif-frame+.h.c
  build/libobs.ported-nsgif-lzw+.h.c
  graphics/libnsgif/gif-blocks.h
  graphics/libnsgif/gif-frame.h
  graphics/libnsgif/lzw.h
  graphics/libnsgif/nsgif.h
  PROPERTIES COMPILE_OPTIONS "/wd4244;/wd4267"
)

# PRIVATE: libobs and libobs-with-rust get these as link-only items, so
# OBS::obfuscate's interface group (build/libobs.obfuscate-windows+.h.c) is compiled
# once, into libobs-objects, not again into each DLL.
target_link_libraries(
  libobs-objects
  PRIVATE Avrt Dwmapi Dxgi winmm Rpcrt4 OBS::obfuscate OBS::winhandle OBS::COMutils
  PUBLIC OBS::w32-pthreads
)
target_link_libraries(libobs INTERFACE OBS::w32-pthreads)

foreach(target IN LISTS LIBOBS_SHARED_TARGETS)
  target_link_options(${target} PRIVATE /IGNORE:4098 /SAFESEH:NO)
  set_target_properties(${target} PROPERTIES PREFIX "" OUTPUT_NAME "obs")
endforeach()

set_property(
  TARGET libobs
  APPEND
  PROPERTY
    OBS_PUBLIC_HEADERS
      util/threading-windows.h
      util/windows/ComPtr.hpp
      util/windows/CoTaskMemPtr.hpp
      util/windows/device-enum.h
      util/windows/HRError.hpp
      util/windows/win-registry.h
      util/windows/win-version.h
      util/windows/window-helpers.h
      util/windows/WinHandle.hpp
)
