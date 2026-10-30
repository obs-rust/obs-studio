cmake_minimum_required(VERSION 3.28...3.30)

include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/common/bootstrap.cmake" NO_POLICY_SCOPE)

project(obs-studio VERSION ${OBS_VERSION_CANONICAL})

# Unity groups, precompiled headers and implementation files include repo
# files by their repo-root path (BUILD.md); also in the Windows child
# architecture builds below, which configure only a few subdirectories.
include_directories("${CMAKE_CURRENT_SOURCE_DIR}")

if(CMAKE_HOST_SYSTEM_NAME MATCHES "Windows")
  include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/windows/architecture.cmake")
  if(NOT OBS_PARENT_ARCHITECTURE STREQUAL CMAKE_VS_PLATFORM_NAME)
    return()
  endif()
endif()

# Product identity is independent of libobs implementation/rollback selection.
# Keep the historical option as a branding-only compatibility input.
option(ENABLE_RUST_LIBOBS "Use OBS-Studio-Rust product identity (deprecated; use OBS_BRAND_RUST)" ON)
option(OBS_BRAND_RUST "Use OBS-Studio-Rust product identity" "${ENABLE_RUST_LIBOBS}")

include(compilerconfig)
include(app-identity)
include(defaults)
include(helpers)
include(cross-targets)

option(ENABLE_FRONTEND "Enable building with UI (requires Qt)" ON)
option(ENABLE_SCRIPTING "Enable scripting support" ON)
option(ENABLE_HEVC "Enable HEVC encoders" ON)

# The Apple frontend links the OS libcurl, whose Brotli decoder this project
# does not pin, so the shared Accept-Encoding helper keeps asking for gzip and
# deflate there (#2211, shared/file-updater/file-updater/curl-accept-encoding.h).
# Measured: macOS 14/15/26 ship libcurl 8.7.1 without CURL_VERSION_BROTLI, so
# there the setting is inert; macOS 13.0 (the deployment target) is unmeasured
# and the reason this stays (#2907).
if(OS_MACOS)
  add_compile_definitions(OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI=1)
endif()

add_subdirectory(libobs)
if(OS_WINDOWS)
  add_subdirectory(libobs-d3d11)
  add_subdirectory(libobs-winrt)
endif()
add_subdirectory(libobs-opengl)
if(OS_MACOS)
  add_subdirectory(libobs-metal)
endif()
add_subdirectory(plugins)

add_subdirectory(test/test-input)

option(ENABLE_UNIT_TESTS "Build and register the cmocka unit tests" OFF)
if(ENABLE_UNIT_TESTS)
  enable_testing()
  add_subdirectory(test/cmocka)
endif()

add_subdirectory(frontend)

message_configuration()
