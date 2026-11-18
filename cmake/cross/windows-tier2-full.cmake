# Full registry, including the legacy fragments and deferred real link support.
# Missing owners remain missing; the inventory auditor reports every omitted
# native record. No empty owner target may turn an optional guard on.
include(CMakePackageConfigHelpers)
# Prebuilt Windows archives name MSVCRT/OLDNAMES in .drectve sections. Their
# existing case aliases must be visible to every canonical owner/test link.
link_directories("${CMAKE_BINARY_DIR}/cross-case-libs")
set(_cmocka_config "${CMAKE_BINARY_DIR}/tier2-cmocka-config")
file(MAKE_DIRECTORY "${_cmocka_config}")
file(
  WRITE "${_cmocka_config}/cmocka-config.cmake"
  "set(CMOCKA_INCLUDE_DIR \"${cmocka_SOURCE_DIR}/include\")\nset(CMOCKA_LIBRARIES cmocka)\n"
)
write_basic_package_version_file(
  "${_cmocka_config}/cmocka-config-version.cmake"
  VERSION 1.1.7
  COMPATIBILITY SameMajorVersion
)
set(CMocka_DIR "${_cmocka_config}")
set(OS_WINDOWS TRUE)
set(OS_MACOS FALSE)
set(OS_LINUX FALSE)
set(OS_FREEBSD FALSE)
set(OS_OPENBSD FALSE)
set(ENABLE_FRONTEND OFF)
set(ENABLE_UI OFF)
set(ENABLE_SCRIPTING OFF)
set(ENABLE_BROWSER OFF)
set(ENABLE_WEBSOCKET OFF)
set(ENABLE_CORE_MODULES ON)
# bootstrap sets this for native directories. The probe's scoped bootstrap
# cannot propagate it, so retain native owner-local header discovery here.
set(CMAKE_INCLUDE_CURRENT_DIR TRUE)
foreach(_variant IN ITEMS shared reference)
  target_include_directories(libobs-cross-${_variant} PRIVATE "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
endforeach()
add_library(libobs-cross-core-modules OBJECT "${CMAKE_BINARY_DIR}/config/obs-core-modules.c")
add_library(OBS::libobs-core-modules ALIAS libobs-cross-core-modules)
target_include_directories(libobs-cross-core-modules PUBLIC "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
set(HAVE_LIBOBS_RUST_STATIC TRUE)
set(OBS_TEST_LIBOBS_SHARED_TARGETS libobs-cross-shared libobs-cross-reference)
add_library(libobs-objects ALIAS libobs-cross-shared-objects)
list(PREPEND CMAKE_PREFIX_PATH "${OBS_CROSS_QT}")
# All imported packages must describe the Windows target, never the Linux host.
set(CMAKE_FIND_ROOT_PATH "${OBS_CROSS_DEPS}" "${OBS_CROSS_QT}")
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(_threads_probe_kind "${CMAKE_TRY_COMPILE_TARGET_TYPE}")
set(_threads_probe_config "${CMAKE_TRY_COMPILE_CONFIGURATION}")
set(CMAKE_TRY_COMPILE_TARGET_TYPE EXECUTABLE)
set(CMAKE_TRY_COMPILE_CONFIGURATION RelWithDebInfo)
foreach(_probe_fact IN ITEMS CMAKE_HAVE_LIBC_PTHREAD CMAKE_HAVE_PTHREADS_CREATE CMAKE_HAVE_PTHREAD_CREATE)
  unset(${_probe_fact} CACHE)
endforeach()
find_package(Threads REQUIRED)
set(CMAKE_TRY_COMPILE_TARGET_TYPE "${_threads_probe_kind}")
set(CMAKE_TRY_COMPILE_CONFIGURATION "${_threads_probe_config}")
target_include_directories(libobs-cross-shared-pthreads PUBLIC "${OBS_SOURCE_ROOT}/deps/w32-pthreads")
target_link_libraries(libobs-cross-shared INTERFACE OBS::w32-pthreads SIMDe::SIMDe Threads::Threads)
configure_file("${OBS_SOURCE_ROOT}/libobs/obsversion.c.in" "${CMAKE_BINARY_DIR}/config/obsversion.c" @ONLY)
# Import target Qt before entering the canonical directory's source-root
# context: Qt schedules deployment bookkeeping on the real top-level directory.
set(QT_HOST_PATH "${OBS_SOURCE_ROOT}/.deps/host-qt6-${OBS_CROSS_QT_VERSION}")
foreach(_tools IN ITEMS Core Gui Widgets)
  set(Qt6${_tools}Tools_DIR "${QT_HOST_PATH}/lib/cmake/Qt6${_tools}Tools" CACHE PATH "Linux Qt generators" FORCE)
endforeach()
find_package(Qt6 QUIET COMPONENTS Core Network Widgets Test)
foreach(_component IN ITEMS avdevice swscale swresample)
  add_library(FFmpeg::${_component} UNKNOWN IMPORTED GLOBAL)
  set_target_properties(
    FFmpeg::${_component}
    PROPERTIES
      IMPORTED_LOCATION "${OBS_CROSS_DEPS}/lib/${_component}.lib"
      INTERFACE_INCLUDE_DIRECTORIES "${OBS_CROSS_DEPS}/include"
  )
endforeach()
add_library(ZLIB::ZLIB UNKNOWN IMPORTED GLOBAL)
set_target_properties(
  ZLIB::ZLIB
  PROPERTIES
    IMPORTED_LOCATION "${OBS_CROSS_DEPS}/lib/zlib.lib"
    INTERFACE_INCLUDE_DIRECTORIES "${OBS_CROSS_DEPS}/include"
)
add_library(jansson::jansson UNKNOWN IMPORTED GLOBAL)
set_target_properties(
  jansson::jansson
  PROPERTIES
    IMPORTED_LOCATION "${OBS_CROSS_DEPS}/lib/jansson.lib"
    INTERFACE_INCLUDE_DIRECTORIES "${OBS_CROSS_DEPS}/include"
)
find_package(MbedTLS REQUIRED)
foreach(
  _shared
  IN
  ITEMS happy-eyeballs opts-parser bpm media-playback file-updater
)
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/${_shared}" "${CMAKE_BINARY_DIR}/shared/${_shared}")
endforeach()
# Runtime renderer products from the same canonical unity/PCH objects. These
# are real linked modules, not interface owners used to satisfy a test guard.
add_subdirectory("${OBS_SOURCE_ROOT}/deps/glad" "${CMAKE_BINARY_DIR}/deps/glad")
foreach(_renderer IN ITEMS d3d11 opengl)
  add_library(libobs-${_renderer} SHARED $<TARGET_OBJECTS:libobs-${_renderer}-cross>)
  add_library(OBS::libobs-${_renderer} ALIAS libobs-${_renderer})
  target_link_libraries(libobs-${_renderer} PRIVATE OBS::libobs)
  set_target_properties(libobs-${_renderer} PROPERTIES PREFIX "")
  function(_obs_windows_tier2_renderer_resource)
    set(CMAKE_CURRENT_SOURCE_DIR "${OBS_SOURCE_ROOT}/libobs-${_renderer}")
    set(CMAKE_SOURCE_DIR "${OBS_SOURCE_ROOT}")
    include(bootstrap)
    configure_file(
      "${OBS_SOURCE_ROOT}/libobs-${_renderer}/cmake/windows/obs-module.rc.in"
      "${CMAKE_BINARY_DIR}/config/libobs-${_renderer}.rc"
    )
  endfunction()
  _obs_windows_tier2_renderer_resource()
  target_sources(libobs-${_renderer} PRIVATE "${CMAKE_BINARY_DIR}/config/libobs-${_renderer}.rc")
endforeach()
target_link_libraries(
  libobs-d3d11
  PRIVATE d3d9 d3d11 d3dcompiler dxgi shcore
)
target_link_libraries(libobs-opengl PRIVATE OBS::glad)
include("${CMAKE_CURRENT_LIST_DIR}/windows-tier2-owners.cmake")
obs_windows_tier2_owners()
# Native fragments expect the top-level OBS roots. Scope these compatibility
# facts to the canonical directory; the actual compiler stays GNU-style Clang.
function(_obs_windows_tier2_full_test_headers)
  get_property(_targets DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_kind "${_target}" TYPE)
    if(_kind MATCHES "^(EXECUTABLE|STATIC_LIBRARY|SHARED_LIBRARY|MODULE_LIBRARY|OBJECT_LIBRARY)$")
      target_include_directories(
        ${_target}
        PRIVATE "${OBS_SOURCE_ROOT}/deps/w32-pthreads" "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config"
      )
      target_include_directories(${_target} SYSTEM PRIVATE "${OBS_CROSS_DEPS}/include")
      get_target_property(_libraries ${_target} LINK_LIBRARIES)
      if(NOT _libraries MATCHES "libobs_rust-static" AND NOT _kind STREQUAL "OBJECT_LIBRARY")
        target_link_libraries(${_target} PRIVATE "${_tier2_builtins}")
      endif()
    endif()
  endforeach()
endfunction()
function(_obs_windows_tier2_full_registry)
  set(CMAKE_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(PROJECT_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(MSVC TRUE)
  # The unchanged stale-manifest fixture starts a single-config nested build.
  # Match the hosted job's RelWithDebInfo instead of requesting xwin's absent
  # debug CRT through CMake's default Windows-GNU Debug configuration.
  file(
    WRITE "${CMAKE_BINARY_DIR}/tier2-fixture-toolchain.cmake"
    "include(\"${CMAKE_TOOLCHAIN_FILE}\")\nset(CMAKE_BUILD_TYPE RelWithDebInfo CACHE STRING \"\" FORCE)\n"
  )
  set(CMAKE_TOOLCHAIN_FILE "${CMAKE_BINARY_DIR}/tier2-fixture-toolchain.cmake")
  file(
    WRITE "${CMAKE_BINARY_DIR}/tier2-cmocka-context.cmake"
    "set(CMAKE_SOURCE_DIR \"${OBS_SOURCE_ROOT}\")\nset(PROJECT_SOURCE_DIR \"${OBS_SOURCE_ROOT}\")\nset(MSVC TRUE)\ncmake_language(DEFER CALL _obs_windows_tier2_full_test_headers)\n"
  )
  set(CMAKE_PROJECT_obs-cmocka_INCLUDE "${CMAKE_BINARY_DIR}/tier2-cmocka-context.cmake")
  add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" "${CMAKE_BINARY_DIR}/test/cmocka")
endfunction()
enable_testing()
_obs_windows_tier2_full_registry()
include("${CMAKE_CURRENT_LIST_DIR}/windows-tier2-options.cmake")
obs_windows_tier2_driver_options("${CMAKE_CURRENT_SOURCE_DIR}")
add_custom_target(
  tier2-cross-full
  DEPENDS
    libobs-cross-shared
    libobs-cross-reference
    cmocka-tests
    libobs-d3d11
    libobs-opengl
    obs-outputs
    obs-filters
    image-source
    obs-transitions
    text-freetype2
    win-capture
    graphics-hook
    get-graphics-offsets
    inject-helper
    libobs-winrt
)
foreach(
  _owner
  IN
  ITEMS
    obs-outputs
    obs-filters
    image-source
    obs-transitions
    text-freetype2
    win-capture
    graphics-hook
    get-graphics-offsets
    inject-helper
    libobs-winrt
)
  string(APPEND _owner_runtime "$<TARGET_FILE:${_owner}>\n")
endforeach()
get_property(_test_targets DIRECTORY "${OBS_SOURCE_ROOT}/test/cmocka" PROPERTY BUILDSYSTEM_TARGETS)
set(
  _runtime
  "${_owner_runtime}$<TARGET_FILE:libobs-cross-shared>\n$<TARGET_FILE:libobs-cross-reference>\n$<TARGET_FILE:libobs-d3d11>\n$<TARGET_FILE:libobs-opengl>\n"
)
foreach(_target IN LISTS _test_targets)
  get_target_property(_kind "${_target}" TYPE)
  if(_kind MATCHES "^(EXECUTABLE|SHARED_LIBRARY|MODULE_LIBRARY)$")
    string(APPEND _runtime "$<TARGET_FILE:${_target}>\n")
  endif()
endforeach()
file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/tier2-runtime.txt" CONTENT "${_runtime}")

obs_windows_tier2_native_layout()
include("${CMAKE_CURRENT_LIST_DIR}/windows-tier2-relocation.cmake")
cmake_language(DEFER CALL obs_windows_tier2_relocate "${CMAKE_CURRENT_SOURCE_DIR}")
