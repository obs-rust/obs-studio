cmake_minimum_required(VERSION 3.28...3.30)

add_library(libobs-opengl SHARED)
add_library(OBS::libobs-opengl ALIAS libobs-opengl)

if(NOT TARGET OBS::glad)
  add_subdirectory("${CMAKE_SOURCE_DIR}/deps/glad" "${CMAKE_BINARY_DIR}/deps/glad")
endif()

if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(Libdrm REQUIRED)
  find_package(X11 REQUIRED)
  find_package(XCB REQUIRED XCB)
  find_package(X11_XCB REQUIRED)

  if(ENABLE_WAYLAND)
    find_package(OpenGL REQUIRED EGL)
    find_package(Wayland REQUIRED)
  endif()
endif()

# Unity groups (BUILD.md): build/opengl.<name>+.h.c (.h.m) #includes the
# implementation files (gl-foo.c.h) it compiles, behind one precompiled
# header per group. Platform groups are listed plainly per platform.
include("${CMAKE_CURRENT_SOURCE_DIR}/../cmake/common/graphics-sources.cmake")
obs_graphics_sources(_opengl_sources OPENGL_CORE "${CMAKE_CURRENT_SOURCE_DIR}")
target_sources(libobs-opengl PRIVATE ${_opengl_sources} gl-helpers.h gl-shaderparser.h gl-subsystem.h)
if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  obs_graphics_sources(_opengl_sources OPENGL_LINUX "${CMAKE_CURRENT_SOURCE_DIR}")
  target_sources(libobs-opengl PRIVATE ${_opengl_sources})
  if(ENABLE_WAYLAND)
    obs_graphics_sources(_opengl_sources OPENGL_WAYLAND "${CMAKE_CURRENT_SOURCE_DIR}")
    target_sources(libobs-opengl PRIVATE ${_opengl_sources})
  endif()
elseif(OS_MACOS)
  obs_graphics_sources(_opengl_sources OPENGL_MACOS "${CMAKE_CURRENT_SOURCE_DIR}")
  target_sources(libobs-opengl PRIVATE ${_opengl_sources})
elseif(OS_WINDOWS)
  obs_graphics_sources(_opengl_sources OPENGL_WINDOWS "${CMAKE_CURRENT_SOURCE_DIR}")
  target_sources(libobs-opengl PRIVATE ${_opengl_sources})
endif()

target_compile_definitions(libobs-opengl PRIVATE $<$<PLATFORM_ID:Darwin>:GL_SILENCE_DEPRECATION>)

target_link_libraries(
  libobs-opengl
  PRIVATE
    OBS::libobs
    OBS::glad
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>>"
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,IOSurface.framework>>"
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Libdrm::Libdrm>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:XCB::XCB>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:X11::XCB>
    $<$<AND:$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>,$<BOOL:${ENABLE_WAYLAND}>>:OpenGL::EGL>
    $<$<AND:$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>,$<BOOL:${ENABLE_WAYLAND}>>:Wayland::Egl>
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in libobs-opengl.rc)
  target_sources(libobs-opengl PRIVATE libobs-opengl.rc)
endif()

target_enable_feature(libobs "OpenGL renderer")

set_target_properties_obs(
  libobs-opengl
  PROPERTIES FOLDER core
             VERSION 0
             PREFIX ""
             SOVERSION "${OBS_VERSION_MAJOR}"
)
