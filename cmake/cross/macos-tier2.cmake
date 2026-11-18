# Opt-in Linux-produced macOS runtime graph (#5009). This is infrastructure,
# not permission to replace the native validator's renderer/capture contracts.
set(CMAKE_FIND_PACKAGE_TARGETS_GLOBAL TRUE)
set(CMAKE_OBJC_VISIBILITY_PRESET hidden)
set(CMAKE_OBJCXX_VISIBILITY_PRESET hidden)
foreach(_objects IN ITEMS libobs-cross libobs-objects)
  # Canonical macOS compilerconfig also hides Objective-C implementation
  # symbols. C_VISIBILITY_PRESET does not apply to these translation units.
  set_target_properties(${_objects} PROPERTIES OBJC_VISIBILITY_PRESET hidden OBJCXX_VISIBILITY_PRESET hidden)
endforeach()
include("${OBS_SOURCE_ROOT}/cmake/cross/tier2-registry-evidence.cmake")
include("${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.cmake")
obs_rust_hidden_lists(
  "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.d"
  "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden-base.txt"
  "${CMAKE_BINARY_DIR}/config"
  ON
)
target_link_options(
  libobs
  PRIVATE "LINKER:-unexported_symbols_list,${CMAKE_BINARY_DIR}/config/rust-unexports-macos.txt"
)
set_property(TARGET libobs APPEND PROPERTY LINK_DEPENDS "${CMAKE_BINARY_DIR}/config/rust-unexports-macos.txt")

add_library(libobs-c-reference SHARED)
target_link_libraries(
  libobs-c-reference
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
target_link_directories(libobs-c-reference PRIVATE "${OBS_CROSS_DEPS}/lib")
foreach(_variant IN ITEMS shared reference)
  if(_variant STREQUAL "shared")
    set(_core libobs)
  else()
    set(_core libobs-c-reference)
  endif()
  set_target_properties(
    ${_core}
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
  target_link_options(${_core} PRIVATE "LINKER:-adhoc_codesign")
endforeach()

# Reuse the unchanged native registry, including standalone Rust consumers.
# Its paths are repository-relative and its fragments use CMAKE_SOURCE_DIR.
# Scope that variable to this function instead of copying any test declaration.
set(QT_HOST_PATH "${OBS_SOURCE_ROOT}/.deps/host-${OBS_CROSS_HOST_QT_TAG}")
_obs_cross_uic(_host_uic)
find_package(Qt6 REQUIRED COMPONENTS Core Widgets)
# These executable targets are otherwise imported lazily by AUTOMOC/UIC inside
# a later test directory, after our host-tool binding would have been skipped.
find_package(Qt6CoreTools REQUIRED)
find_package(Qt6WidgetsTools REQUIRED)
include("${OBS_SOURCE_ROOT}/cmake/cross/tier2-host-qt.cmake")
obs_tier2_bind_host_qt("${QT_HOST_PATH}")
file(
  GENERATE OUTPUT "${CMAKE_BINARY_DIR}/tier2-host-tools.txt"
  CONTENT
    "$<TARGET_FILE:Qt6::moc>\n$<TARGET_FILE:Qt6::uic>\n$<TARGET_FILE:Qt6::rcc>\n$<TARGET_FILE:Qt6::cmake_automoc_parser>\n"
)
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
  add_library(OBS::libobs ALIAS libobs)
  add_library(OBS::libobs-core-modules ALIAS libobs-cross-generated)
  target_include_directories(libobs INTERFACE "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
  add_library(libobs_rust-static STATIC IMPORTED GLOBAL)
  set_target_properties(
    libobs_rust-static
    PROPERTIES IMPORTED_LOCATION "${OBS_SOURCE_ROOT}/target/aarch64-apple-darwin/release/liblibobs_rust.a"
  )
  add_dependencies(libobs_rust-static libobs-rust)
  add_custom_target(cargo-build_libobs_rust DEPENDS libobs-rust)
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
  target_link_libraries(libobs INTERFACE SIMDe::SIMDe Threads::Threads)
  find_package(jansson REQUIRED)
  find_package(ZLIB REQUIRED)
  target_link_libraries(libobs-c-reference PRIVATE ZLIB::ZLIB)
  # Configure the real native owner recipes. Their aliases gate the unchanged
  # registry; stand-ins would register tests without their link/data closure.
  include(helpers_common)
  include("${OBS_SOURCE_ROOT}/cmake/macos/helpers.cmake")
  set(CMAKE_INCLUDE_CURRENT_DIR TRUE)
  # SDK package exports belong to native installation. The cross producer
  # transports the built runtime graph and does not install CMake packages.
  function(target_export target)
  endfunction()
  function(set_target_properties_obs target)
    cmake_parse_arguments(PARSE_ARGV 1 _props "" "" "PROPERTIES")
    while(_props_PROPERTIES)
      list(POP_FRONT _props_PROPERTIES _key _value)
      set_property(TARGET ${target} PROPERTY ${_key} "${_value}")
    endwhile()
    get_target_property(_kind ${target} TYPE)
    if(_kind MATCHES "^(EXECUTABLE|SHARED_LIBRARY|MODULE_LIBRARY)$")
      target_link_options(${target} PRIVATE "LINKER:-adhoc_codesign")
    endif()
    get_target_property(_arc ${target} XCODE_ATTRIBUTE_CLANG_ENABLE_OBJC_ARC)
    if(_arc)
      target_compile_options(${target} PRIVATE "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fobjc-arc>")
    endif()
    get_target_property(_modules ${target} XCODE_ATTRIBUTE_CLANG_ENABLE_MODULES)
    if(_modules)
      target_compile_options(${target} PRIVATE "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fmodules>")
    endif()
    if(_kind STREQUAL "MODULE_LIBRARY")
      set_target_properties(
        ${target}
        PROPERTIES BUNDLE TRUE BUNDLE_EXTENSION plugin MACOSX_BUNDLE_GUI_IDENTIFIER "com.obsproject.${target}"
      )
      target_link_options(${target} PRIVATE "LINKER:-adhoc_codesign")
      if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/data")
        add_custom_command(
          TARGET ${target}
          POST_BUILD
          COMMAND
            "${CMAKE_COMMAND}" -E copy_directory "${CMAKE_CURRENT_SOURCE_DIR}/data"
            "$<TARGET_BUNDLE_CONTENT_DIR:${target}>/Resources"
        )
      endif()
    endif()
  endfunction()
  add_subdirectory("${OBS_SOURCE_ROOT}/frontend/api" "${CMAKE_BINARY_DIR}/frontend/api")
  # The native plugin dispatcher owns platform/option selection and records
  # enabled modules through target_enable. Do not synthesize its name list.
  # This configures plugins/obs-outputs (RTMP/AMF and happy-eyeballs),
  # shared/media-playback, shared/file-updater, plugins/image-source,
  # plugins/obs-transitions, plugins/obs-filters and plugins/text-freetype2
  # along with the other genuinely enabled native owners.
  add_library(libobs-core-modules ALIAS libobs-cross-generated)
  function(set_obs_core_modules)
    get_property(_enabled GLOBAL PROPERTY OBS_MODULES_ENABLED)
    list(LENGTH _enabled OBS_CORE_MODULE_COUNT)
    if(NOT OBS_CORE_MODULE_COUNT)
      message(FATAL_ERROR "Tier 2 requires the enabled native core-module graph")
    endif()
    string(REPLACE ";" "\",\n\t\"" _array "${_enabled}")
    set(OBS_CORE_MODULE_LIST "\t\"${_array}\"")
    configure_file(
      "${OBS_SOURCE_ROOT}/libobs/obs-core-modules.c.in"
      "${CMAKE_BINARY_DIR}/config/obs-core-modules.c"
      @ONLY
    )
    string(REPLACE ";" "\n" _lines "${_enabled}")
    file(WRITE "${CMAKE_BINARY_DIR}/tier2-modules.txt" "${_lines}\n")
  endfunction()
  add_subdirectory("${OBS_SOURCE_ROOT}/plugins" "${CMAKE_BINARY_DIR}/plugins")
  add_subdirectory("${OBS_SOURCE_ROOT}/libobs-opengl" "${CMAKE_BINARY_DIR}/libobs-opengl")
  target_link_libraries(libobs-opengl PRIVATE "-framework OpenGL")
  if(NOT OBS_TIER2_DARWIN_RECEIPT OR NOT EXISTS "${OBS_TIER2_DARWIN_RECEIPT}")
    message(FATAL_ERROR "Tier 2 requires ci/macos_tier2_toolchain.py and soldr toolchain darwin (SDK 26.5)")
  endif()
  add_custom_target(
    tier2-cross-metal
    COMMAND
      "${CMAKE_COMMAND}" -S "${OBS_SOURCE_ROOT}/cmake/cross/metal" -B "${CMAKE_BINARY_DIR}/libobs-metal-managed" -G
      Ninja "-DCMAKE_TOOLCHAIN_FILE=${CMAKE_TOOLCHAIN_FILE}" "-DCMAKE_BUILD_TYPE=RelWithDebInfo"
      "-DOBS_SOURCE_ROOT=${OBS_SOURCE_ROOT}" "-DOBS_TIER2_CORE=$<TARGET_FILE:libobs>"
      "-DOBS_TIER2_CONFIG=${CMAKE_BINARY_DIR}/config" "-DOBS_TIER2_DEPS=${OBS_CROSS_DEPS}"
    COMMAND "${CMAKE_COMMAND}" --build "${CMAKE_BINARY_DIR}/libobs-metal-managed" --target libobs-metal
    BYPRODUCTS "${CMAKE_BINARY_DIR}/libobs-metal-managed/libobs-metal.0.dylib"
    DEPENDS libobs
    COMMENT "Cross-build the canonical Swift Metal recipe on Linux"
  )
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
  enable_testing()
  add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" "${CMAKE_BINARY_DIR}/test/cmocka")
endfunction()
_obs_cross_tier2_tests()
enable_testing()
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
add_custom_target(libobs-cross-shared DEPENDS libobs)
add_custom_target(libobs-cross-reference DEPENDS libobs-c-reference)
add_custom_target(
  tier2-cross
  DEPENDS
    libobs
    libobs-c-reference
    cmocka-tests
    libobs-opengl
    tier2-cross-manifest-fixture
    tier2-cross-metal
    mac-capture
    mac-avcapture
    mac-avcapture-legacy
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
set(_runtime_paths "${CMAKE_BINARY_DIR}/libobs-metal-managed/libobs-metal.0.dylib\n")
foreach(_target IN LISTS _runtime_targets)
  string(APPEND _runtime_paths "$<TARGET_FILE:${_target}>\n")
endforeach()
file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/tier2-runtime.txt" CONTENT "${_runtime_paths}")
