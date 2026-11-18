# Opt-in Linux-produced macOS runtime graph (#5009). This is infrastructure,
# not permission to replace the native validator's renderer/capture contracts.
include("${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.cmake")
obs_rust_hidden_lists(
  "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.d"
  "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden-base.txt"
  "${CMAKE_BINARY_DIR}/config"
  ON
)
target_link_options(
  libobs-cross-shared
  PRIVATE "LINKER:-unexported_symbols_list,${CMAKE_BINARY_DIR}/config/rust-unexports-macos.txt"
)
set_property(
  TARGET libobs-cross-shared
  APPEND
  PROPERTY LINK_DEPENDS "${CMAKE_BINARY_DIR}/config/rust-unexports-macos.txt"
)

add_library(libobs-cross-reference SHARED)
target_link_libraries(
  libobs-cross-reference
  PRIVATE
    libobs-cross
    OBS::caption
    libobs-cross-generated
    avcodec
    avformat
    avutil
    swscale
    swresample
    jansson
    "-framework AppKit"
    "-framework AudioToolbox"
    "-framework AudioUnit"
    "-framework Carbon"
    "-framework Cocoa"
    "-framework CoreAudio"
    "-framework IOKit"
)
target_link_directories(libobs-cross-reference PRIVATE "${OBS_CROSS_DEPS}/lib")
foreach(_variant IN ITEMS shared reference)
  set_target_properties(
    libobs-cross-${_variant}
    PROPERTIES
      FRAMEWORK TRUE
      FRAMEWORK_VERSION A
      OUTPUT_NAME libobs
      PREFIX ""
      LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/cores/${_variant}"
      INSTALL_NAME_DIR "@rpath"
      BUILD_WITH_INSTALL_NAME_DIR TRUE
      INSTALL_RPATH "@loader_path/../../../../../../deps"
      BUILD_WITH_INSTALL_RPATH TRUE
  )
  target_link_options(libobs-cross-${_variant} PRIVATE "LINKER:-adhoc_codesign")
endforeach()

# Reuse the unchanged native registry, including standalone Rust consumers.
# Its paths are repository-relative and its fragments use CMAKE_SOURCE_DIR.
# Scope that variable to this function instead of copying any test declaration.
set(QT_HOST_PATH "${OBS_SOURCE_ROOT}/.deps/host-${OBS_CROSS_HOST_QT_TAG}")
find_package(Qt6 REQUIRED COMPONENTS Core Widgets)
foreach(_tool IN ITEMS moc uic rcc)
  if(TARGET Qt6::${_tool})
    set_target_properties(
      Qt6::${_tool}
      PROPERTIES
        IMPORTED_LOCATION "${QT_HOST_PATH}/libexec/${_tool}"
        IMPORTED_LOCATION_RELEASE "${QT_HOST_PATH}/libexec/${_tool}"
    )
  endif()
endforeach()
function(_obs_cross_tier2_tests)
  set(CMAKE_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(OS_MACOS TRUE)
  set(OS_LINUX FALSE)
  set(ENABLE_FRONTEND OFF)
  set(ENABLE_UI OFF)
  set(ENABLE_CORE_MODULES ON)
  set(ENABLE_SCRIPTING OFF)
  set(ENABLE_BROWSER OFF)
  set(ENABLE_WEBSOCKET OFF)
  set(CMAKE_BUILD_WITH_INSTALL_RPATH TRUE)
  set(CMAKE_INSTALL_RPATH "@loader_path;@loader_path/../../../deps")
  set(OBS_VERSION_MAJOR 32)
  list(PREPEND CMAKE_MODULE_PATH "${OBS_SOURCE_ROOT}/cmake/finders")
  list(PREPEND CMAKE_PREFIX_PATH "${OBS_CROSS_DEPS}")
  set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
  set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
  list(APPEND CMAKE_FIND_ROOT_PATH "${OBS_CROSS_DEPS}")
  include_directories("${OBS_SOURCE_ROOT}" "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
  add_library(OBS::libobs ALIAS libobs-cross-shared)
  add_library(libobs-objects ALIAS libobs-cross-shared-objects)
  add_library(OBS::libobs-core-modules ALIAS libobs-cross-generated)
  target_include_directories(libobs-cross-shared INTERFACE "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
  add_library(libobs_rust-static STATIC IMPORTED GLOBAL)
  set_target_properties(
    libobs_rust-static
    PROPERTIES IMPORTED_LOCATION "${OBS_SOURCE_ROOT}/target/aarch64-apple-darwin/release/liblibobs_rust.a"
  )
  add_dependencies(libobs_rust-static libobs-cross-shared-rust)
  add_custom_target(cargo-build_libobs_rust DEPENDS libobs-cross-shared-rust)
  target_link_libraries(
    libobs_rust-static
    INTERFACE "-framework CoreFoundation" "-framework Foundation" "-framework IOKit" "-framework Security" iconv
  )
  foreach(
    _component
    IN
    ITEMS avcodec avformat avutil swscale swresample
  )
    find_library(_cross_${_component} NAMES ${_component} PATHS "${OBS_CROSS_DEPS}/lib" NO_DEFAULT_PATH REQUIRED)
    add_library(FFmpeg::${_component} UNKNOWN IMPORTED GLOBAL)
    set_target_properties(
      FFmpeg::${_component}
      PROPERTIES IMPORTED_LOCATION "${_cross_${_component}}" INTERFACE_INCLUDE_DIRECTORIES "${OBS_CROSS_DEPS}/include"
    )
  endforeach()
  find_package(Uthash REQUIRED)
  find_package(SIMDe REQUIRED)
  find_package(Threads REQUIRED)
  target_link_libraries(libobs-cross-shared INTERFACE SIMDe::SIMDe Threads::Threads)
  find_package(jansson REQUIRED)
  find_package(ZLIB REQUIRED)
  target_link_libraries(libobs-cross-reference PRIVATE ZLIB::ZLIB)
  # The run host must not compile cmocka. Build the pinned source for Darwin.
  include(FetchContent)
  set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build cmocka into the test executables" FORCE)
  set(WITH_EXAMPLES OFF CACHE BOOL "" FORCE)
  set(UNIT_TESTING OFF CACHE BOOL "" FORCE)
  FetchContent_Declare(
    cmocka
    URL https://cmocka.org/files/1.1/cmocka-1.1.7.tar.xz
    URL_HASH SHA256=810570eb0b8d64804331f82b29ff47c790ce9cd6b163e98d47a4807047ecad82
    DOWNLOAD_EXTRACT_TIMESTAMP TRUE
  )
  FetchContent_MakeAvailable(cmocka)
  set(CMOCKA_INCLUDE_DIR "${cmocka_SOURCE_DIR}/include")
  set(CMOCKA_LIBRARIES cmocka)
  # find_package in the unchanged registry consumes this build-tree config.
  file(WRITE "${CMAKE_BINARY_DIR}/cmocka-config/cmocka-config.cmake" "set(CMocka_FOUND TRUE)\n")
  file(
    WRITE "${CMAKE_BINARY_DIR}/cmocka-config/cmocka-config-version.cmake"
    "set(PACKAGE_VERSION 1.1.7)\nset(PACKAGE_VERSION_COMPATIBLE TRUE)\n"
  )
  set(CMocka_DIR "${CMAKE_BINARY_DIR}/cmocka-config")
  add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" "${CMAKE_BINARY_DIR}/test/cmocka")
endfunction()
enable_testing()
_obs_cross_tier2_tests()
add_custom_target(
  tier2-cross-manifest-fixture
  COMMAND
    "${CMAKE_COMMAND}" "-DOBS_CORE_MODULES_CMAKE=${OBS_SOURCE_ROOT}/libobs/cmake/obs-core-modules.cmake"
    "-DTEST_FIXTURE_ROOT=${CMAKE_BINARY_DIR}/test/cmocka/core_module_manifest_source"
    "-DTEST_GENERATOR=${CMAKE_GENERATOR}" "-DTEST_C_COMPILER=${CMAKE_C_COMPILER}"
    "-DTEST_MAKE_PROGRAM=${CMAKE_MAKE_PROGRAM}" "-DTEST_TOOLCHAIN_FILE=${CMAKE_TOOLCHAIN_FILE}"
    "-DTEST_OSX_ARCHITECTURES=${CMAKE_OSX_ARCHITECTURES}" -DTEST_PREPARE_ONLY=ON -P
    "${OBS_SOURCE_ROOT}/test/cmocka/check_core_module_manifest_source.cmake"
  COMMENT "Prepare stale core-manifest fixture on the Linux producer"
)
add_custom_target(
  tier2-cross
  DEPENDS libobs-cross-shared libobs-cross-reference cmocka-tests tier2-cross-manifest-fixture
)

# Export actual runtime targets, including generated/loadable fixture libraries.
# Never derive this list from executable-looking filenames in a build directory.
function(_obs_cross_runtime_targets directory)
  get_property(_targets DIRECTORY "${directory}" PROPERTY BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_type ${_target} TYPE)
    if(_type MATCHES "^(EXECUTABLE|SHARED_LIBRARY|MODULE_LIBRARY)$")
      set_property(GLOBAL APPEND PROPERTY OBS_TIER2_RUNTIME_TARGETS ${_target})
    endif()
  endforeach()
  get_property(_children DIRECTORY "${directory}" PROPERTY SUBDIRECTORIES)
  foreach(_child IN LISTS _children)
    _obs_cross_runtime_targets("${_child}")
  endforeach()
endfunction()
_obs_cross_runtime_targets("${CMAKE_SOURCE_DIR}")
get_property(_runtime_targets GLOBAL PROPERTY OBS_TIER2_RUNTIME_TARGETS)
set(_runtime_paths "")
foreach(_target IN LISTS _runtime_targets)
  string(APPEND _runtime_paths "$<TARGET_FILE:${_target}>\n")
endforeach()
file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/tier2-runtime.txt" CONTENT "${_runtime_paths}")
add_dependencies(tier2-cross ${_runtime_targets})
