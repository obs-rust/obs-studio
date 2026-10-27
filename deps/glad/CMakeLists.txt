cmake_minimum_required(VERSION 3.28...3.30)

find_package(OpenGL REQUIRED)

add_library(obsglad OBJECT)
add_library(OBS::glad ALIAS obsglad)

include("${CMAKE_CURRENT_SOURCE_DIR}/../../cmake/common/graphics-sources.cmake")
obs_graphics_sources(_glad_sources GLAD_CORE "${CMAKE_CURRENT_SOURCE_DIR}")
if(CMAKE_SYSTEM_NAME STREQUAL "Windows")
  obs_graphics_sources(_glad_windows GLAD_WINDOWS "${CMAKE_CURRENT_SOURCE_DIR}")
  list(APPEND _glad_sources ${_glad_windows})
endif()
if(TARGET OpenGL::EGL)
  obs_graphics_sources(_glad_egl GLAD_EGL "${CMAKE_CURRENT_SOURCE_DIR}")
  list(APPEND _glad_sources ${_glad_egl})
endif()

target_sources(
  obsglad
  PRIVATE ${_glad_sources} $<$<TARGET_EXISTS:OpenGL::EGL>:include/EGL/eglplatform.h>
  PUBLIC
    include/glad/glad.h
    "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/include/glad/glad_wgl.h>"
    "$<$<TARGET_EXISTS:OpenGL::EGL>:${CMAKE_CURRENT_SOURCE_DIR}/include/glad/glad_egl.h>"
)

# Vendored loader: casts between object and function pointers throughout, which
# -Wpedantic rejects. Exempt the target, as #2623 asks, not the whole build.
target_compile_options(
  obsglad
  PRIVATE
    $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-strict-prototypes>
    $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-pedantic>
)

# Generated GL/WGL loaders share a tentative C global; clang-cl otherwise
# rejects the separate objects instead of matching MSVC's common symbols.
if(CMAKE_C_COMPILER_ID STREQUAL "Clang" AND CMAKE_C_SIMULATE_ID STREQUAL "MSVC")
  target_compile_options(obsglad PRIVATE /clang:-fcommon)
endif()

target_include_directories(obsglad PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")

target_link_libraries(
  obsglad
  PRIVATE $<$<NOT:$<PLATFORM_ID:Windows,Darwin>>:${CMAKE_DL_LIBS}>
  PUBLIC OpenGL::GL $<$<TARGET_EXISTS:OpenGL::EGL>:OpenGL::EGL>
)

set_target_properties(obsglad PROPERTIES FOLDER deps POSITION_INDEPENDENT_CODE TRUE)
