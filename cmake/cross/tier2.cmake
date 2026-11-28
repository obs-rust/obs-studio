# Link the real Tier 2 registrations; keep their host conditions, archive order
# and deferred support hooks. No CTest execution takes place on the Linux host.
set(CMAKE_INCLUDE_CURRENT_DIR TRUE)
set(ENABLE_FRONTEND OFF)
set(CMAKE_DISABLE_FIND_PACKAGE_Qt6 TRUE)
set(ENABLE_HEVC ON)
set(ENABLE_CORE_MODULES ON)
set(ENABLE_WAYLAND OFF)
set(OS_${_host} TRUE)
if(OS_WINDOWS AND NOT MSVC)
  message(FATAL_ERROR "Tier 2 Windows links require clang-cl; use ci/cmocka_link_check.py")
endif()
list(APPEND CMAKE_MODULE_PATH "${OBS_SOURCE_ROOT}/cmake/finders")
include(helpers_common)
enable_testing()

include(FetchContent)
FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.5.2)
FetchContent_MakeAvailable(Corrosion)
corrosion_import_crate(
  MANIFEST_PATH "${OBS_SOURCE_ROOT}/Cargo.toml"
  CRATES libobs-rust
  CRATE_TYPES staticlib
  PROFILE release
)
target_link_libraries(libobs_rust-static INTERFACE OBS::libobs-native-dependencies)

if(OS_MACOS)
  # Imported @rpath dylibs (MbedTLS) need the Darwin runtime-path flag, which
  # CMake's platform files only define for a host-native Darwin build.
  set(CMAKE_SHARED_LIBRARY_RUNTIME_C_FLAG "-Wl,-rpath,")
  set(CMAKE_SHARED_LIBRARY_RUNTIME_C_FLAG_SEP ":")
  # A host librt must never satisfy a Darwin dependency probe.
  list(APPEND CMAKE_FIND_ROOT_PATH "${OBS_CROSS_DEPS}")
  set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
  unset(RT_LIBRARY CACHE)
  unset(CMOCKA_REQUIRED_LIBRARIES CACHE)
  unset(CMOCKA_LINK_LIBRARIES CACHE)
  # These are the crate-declared SDK imports in obs-util's cocoa.rs. Corrosion
  # 0.5 imports the standard-library link set, not these framework annotations.
  target_link_libraries(
    libobs_rust-static
    INTERFACE "-framework Foundation" "-framework CoreFoundation" "-framework IOKit" objc
  )
  # Match production visibility: exporting private Rust shims from the dylib
  # could otherwise conceal a missing standalone archive/support dependency.
  include("${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.cmake")
  obs_rust_hidden_lists(
    "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden.d"
    "${OBS_SOURCE_ROOT}/libobs/cmake/rust-hidden-base.txt"
    "${CMAKE_BINARY_DIR}/config"
    "${ENABLE_HEVC}"
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
endif()

find_package(SIMDe REQUIRED)
find_package(Uthash REQUIRED)
find_package(Threads REQUIRED)
add_library(OBS::libobs ALIAS libobs-cross-shared)
add_library(libobs-objects ALIAS libobs-cross-shared-objects)
add_library(OBS::libobs-core-modules ALIAS libobs-cross-generated)
target_include_directories(libobs-cross-shared PUBLIC "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
# The real libobs exposes SIMDe to every consumer (libobs/CMakeLists.txt).
target_link_libraries(libobs-cross-shared INTERFACE SIMDe::SIMDe Threads::Threads)
# OBS::libobs-core-modules publishes obs-core-modules.h from the libobs directory.
target_include_directories(libobs-cross-generated PUBLIC "${OBS_SOURCE_ROOT}/libobs")
# Rust object directives retain Windows case on Linux (MSVCRT/OLDNAMES).
if(OS_WINDOWS)
  link_directories("${CMAKE_BINARY_DIR}/cross-case-libs")
  # Rust libc calls external snprintf; the UCRT C headers inline it. Use
  # the real SDK compatibility archive when linking with clang-cl/LLD.
  link_libraries(legacy_stdio_definitions)
  file(
    CREATE_LINK "${CMAKE_FIND_ROOT_PATH}/sdk/lib/um/x86_64/rpcrt4.lib" "${CMAKE_BINARY_DIR}/cross-case-libs/Rpcrt4.lib"
    SYMBOLIC
  )
  target_include_directories(libobs-cross-shared PUBLIC "${OBS_SOURCE_ROOT}/deps/w32-pthreads")
  add_library(OBS::w32-pthreads ALIAS libobs-cross-shared-pthreads)
  target_include_directories(libobs-cross-shared-pthreads PUBLIC "${OBS_SOURCE_ROOT}/deps/w32-pthreads")
  # As libobs/cmake/os-windows.cmake does: consumers of libobs get w32-pthreads'
  # import library. Tests that compile libobs sources into themselves
  # (test_threading, the *_seam and *_steps tests) call pthread_* directly.
  target_link_libraries(libobs-cross-shared INTERFACE OBS::w32-pthreads)
endif()
find_package(
  FFmpeg
  8.0
  REQUIRED
  COMPONENTS avcodec avdevice avformat avutil swscale swresample
)
find_package(jansson REQUIRED)
find_package(Uthash REQUIRED)
# libobs links uthash's headers into every consumer (libobs/CMakeLists.txt).
target_link_libraries(libobs-cross-shared INTERFACE Uthash::Uthash)
find_package(SIMDe REQUIRED)
find_package(ZLIB REQUIRED)

# Same pinned cmocka as the Darwin run-only producer. Build for the target;
# a host cmocka or a placeholder library would conceal unresolved imports.
FetchContent_Declare(
  cmocka
  URL https://cmocka.org/files/1.1/cmocka-1.1.7.tar.xz
  URL_HASH SHA256=810570eb0b8d64804331f82b29ff47c790ce9cd6b163e98d47a4807047ecad82
)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "Cross-built cmocka is static" FORCE)
set(UNIT_TESTING OFF)
set(WITH_EXAMPLES OFF)
set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
# Function-existence probes must link: a static try-compile would claim POSIX
# sigsetjmp/siglongjmp exist in the Windows SDK merely from declarations.
set(CMAKE_TRY_COMPILE_TARGET_TYPE EXECUTABLE)
find_package(Threads REQUIRED)
if(OS_WINDOWS)
  unset(HAVE_SIGSETJMP CACHE)
  unset(HAVE_SIGLONGJMP CACHE)
endif()
FetchContent_MakeAvailable(cmocka)
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)
set_target_properties(cmocka PROPERTIES OBS_DISABLE_PCH TRUE OBS_DISABLE_UNITY_BUILD TRUE)
set(CMOCKA_INCLUDE_DIR "${cmocka_SOURCE_DIR}/include")
set(CMOCKA_LIBRARIES cmocka)
set(CMocka_DIR "${CMAKE_CURRENT_LIST_DIR}/cmocka")
if(OS_MACOS)
  # The explicit C reference backend (OFF suite): the C ports without the Rust
  # halves, under the hybrid's file name and install name so DYLD_LIBRARY_PATH
  # selects which libobs the unchanged tests load (libobs/cmake/implementation.cmake).
  add_library(libobs-cross-reference SHARED)
  target_link_libraries(
    libobs-cross-reference
    PRIVATE libobs-cross OBS::caption libobs-cross-generated libobs-cross-shared-native ZLIB::ZLIB
  )
  target_include_directories(libobs-cross-reference PUBLIC "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config")
  target_link_libraries(libobs-cross-reference INTERFACE SIMDe::SIMDe Threads::Threads)
  set(_obs_core_layouts "libobs-cross-shared=libobs" "libobs-cross-reference=libobs-c-reference")
  foreach(_layout IN LISTS _obs_core_layouts)
    string(REPLACE "=" ";" _layout_parts "${_layout}")
    list(GET _layout_parts 0 _core_target)
    list(GET _layout_parts 1 _core_directory)
    set_target_properties(
      ${_core_target}
      PROPERTIES
        FRAMEWORK TRUE
        FRAMEWORK_VERSION A
        PREFIX ""
        OUTPUT_NAME libobs
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/${_core_directory}"
        INSTALL_NAME_DIR "@rpath"
        BUILD_WITH_INSTALL_NAME_DIR TRUE
    )
  endforeach()
endif()
# macOS registry parity only: the Windows graph keeps its own, smaller registry until
# #5864's case-folding clang-cl driver lets the librtmp tests build there (#5870).
if(OS_MACOS)
  # The shared owners the unchanged registry gates on (OBS::happy-eyeballs,
  # OBS::media-playback); their own recipes, not stand-ins.
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/happy-eyeballs" "${CMAKE_BINARY_DIR}/shared/happy-eyeballs")
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/media-playback" "${CMAKE_BINARY_DIR}/shared/media-playback")
  # Core module recipes that gate registrations by target (obs-outputs, image-source,
  # obs-transitions, text-freetype2, obs-filters). Registered tests compile the
  # module sources directly, so the module bundles themselves are never built here.
  set(ENABLE_FREETYPE ON)
  # Packages found inside a plugin recipe must stay visible to the test registry.
  set(CMAKE_FIND_PACKAGE_TARGETS_GLOBAL TRUE)
  function(set_target_properties_obs target)
    # ARGV keeps empty values (PREFIX ""), which an unquoted ${ARGN} expansion drops.
    math(EXPR _last "${ARGC} - 1")
    foreach(_name_index RANGE 2 ${_last} 2)
      math(EXPR _value_index "${_name_index} + 1")
      set_property(TARGET ${target} PROPERTY "${ARGV${_name_index}}" "${ARGV${_value_index}}")
    endforeach()
  endfunction()
  # Xcode-only project settings (Info.plist keys); ARC and modules are compile flags here.
  function(set_target_xcode_properties target)
  endfunction()
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/opts-parser" "${CMAKE_BINARY_DIR}/shared/opts-parser")
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/file-updater" "${CMAKE_BINARY_DIR}/shared/file-updater")
  add_subdirectory("${OBS_SOURCE_ROOT}/shared/bpm" "${CMAKE_BINARY_DIR}/shared/bpm")
  set(
    OBS_MODULES_CONFIGURED
    image-source
    obs-transitions
    text-freetype2
    obs-filters
    obs-outputs
  )
  foreach(_plugin IN LISTS OBS_MODULES_CONFIGURED)
    add_subdirectory("${OBS_SOURCE_ROOT}/plugins/${_plugin}" "${CMAKE_BINARY_DIR}/plugins/${_plugin}" EXCLUDE_FROM_ALL)
  endforeach()
  # The linked core-module manifest lists the modules configured here (the
  # dispatcher's full macOS set needs every plugin's dependencies), as
  # set_obs_core_modules() would from OBS_MODULES_ENABLED.
  list(LENGTH OBS_MODULES_CONFIGURED OBS_CORE_MODULE_COUNT)
  string(REPLACE ";" "\",\n\t\"" _core_modules_array "${OBS_MODULES_CONFIGURED}")
  set(OBS_CORE_MODULE_LIST "\t\"${_core_modules_array}\"")
  configure_file(
    "${OBS_SOURCE_ROOT}/libobs/obs-core-modules.c.in"
    "${CMAKE_BINARY_DIR}/config/obs-core-modules.c"
    @ONLY
  )
  # obs-build reads the reference core-module manifest from <build>/libobs (core_module_manifest.rs).
  configure_file(
    "${OBS_SOURCE_ROOT}/libobs/obs-core-modules.c.in"
    "${CMAKE_BINARY_DIR}/libobs/obs-core-modules.c"
    @ONLY
  )
endif()
if(OS_MACOS)
  # Qt Core/Widgets tests: Darwin libraries from the pinned prebuilt, generators
  # (moc/uic/rcc) from the pinned Linux host Qt.
  list(PREPEND CMAKE_PREFIX_PATH "${OBS_CROSS_QT}")
  _obs_cross_uic(_host_uic)
  set(QT_HOST_PATH "${OBS_SOURCE_ROOT}/.deps/host-${OBS_CROSS_HOST_QT_TAG}")
  unset(CMAKE_DISABLE_FIND_PACKAGE_Qt6)
  find_package(Qt6 REQUIRED COMPONENTS Core Widgets)
  find_package(Qt6CoreTools REQUIRED)
  find_package(Qt6WidgetsTools REQUIRED)
  include("${CMAKE_CURRENT_LIST_DIR}/tier2-host-qt.cmake")
  obs_tier2_bind_host_qt("${QT_HOST_PATH}")
endif()
if(OS_MACOS)
  # macOS has no xvfb; the Linux producer's xvfb-run must never wrap the tests it
  # registers. Hide the host directory that holds it from find_program (an empty
  # cache value does not stop the search).
  find_program(_host_xvfb_run xvfb-run)
  if(_host_xvfb_run)
    get_filename_component(_host_xvfb_dir "${_host_xvfb_run}" DIRECTORY)
    list(APPEND CMAKE_IGNORE_PATH "${_host_xvfb_dir}")
  endif()
  # Merged-usr hosts expose one xvfb-run as /bin and /usr/bin; hide every spelling.
  foreach(
    _host_dir
    IN
    ITEMS /bin /usr/bin /usr/local/bin /sbin /usr/sbin
  )
    if(EXISTS "${_host_dir}/xvfb-run")
      list(APPEND CMAKE_IGNORE_PATH "${_host_dir}")
    endif()
  endforeach()
  unset(_host_xvfb_run CACHE)
endif()
if(OS_MACOS)
  # The graphics and capture modules the Cargo-native contract compares against
  # (#5848): the real recipes, linked to the framework-named core above.
  add_subdirectory("${OBS_SOURCE_ROOT}/libobs-opengl" "${CMAKE_BINARY_DIR}/libobs-opengl")
  add_subdirectory("${OBS_SOURCE_ROOT}/plugins/mac-capture" "${CMAKE_BINARY_DIR}/plugins/mac-capture")
  add_subdirectory("${OBS_SOURCE_ROOT}/plugins/mac-avcapture" "${CMAKE_BINARY_DIR}/plugins/mac-avcapture")
  # XCODE_ATTRIBUTE_CLANG_ENABLE_OBJC_ARC YES: the camera modules only; opengl and
  # mac-capture use manual retain/release.
  foreach(_arc_target IN ITEMS mac-avcapture mac-avcapture-legacy)
    target_compile_options(${_arc_target} PRIVATE "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fobjc-arc>")
  endforeach()
  # XCODE_ATTRIBUTE_CLANG_ENABLE_MODULES YES (and AUTOLINK) for mac-avcapture.
  target_compile_options(
    mac-avcapture
    PRIVATE
      "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fmodules>"
      "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fmodules-cache-path=${CMAKE_BINARY_DIR}/modules-cache>"
  )
  foreach(_capture IN ITEMS mac-capture mac-avcapture mac-avcapture-legacy)
    # Native bundles keep the module in Contents/MacOS with its data in Contents/Resources.
    get_target_property(_capture_source_dir ${_capture} SOURCE_DIR)
    set(_bundle "${CMAKE_BINARY_DIR}/plugins/${_capture}/${_capture}.plugin/Contents")
    add_custom_target(
      ${_capture}-bundle
      COMMAND "${CMAKE_COMMAND}" -E make_directory "${_bundle}/MacOS" "${_bundle}/Resources"
      COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:${_capture}>" "${_bundle}/MacOS/${_capture}"
      COMMAND "${CMAKE_COMMAND}" -E copy_directory "${_capture_source_dir}/data" "${_bundle}/Resources"
      VERBATIM
    )
    add_dependencies(${_capture}-bundle ${_capture})
  endforeach()
  # libobs-metal is Swift 6 with an Objective-C bridging header: compiled and linked
  # by soldr's swiftc (the Darwin receipt) against the hybrid core.
  include("${OBS_SOURCE_ROOT}/cmake/common/graphics-sources.cmake")
  set(_metal_dir "${OBS_SOURCE_ROOT}/libobs-metal")
  obs_graphics_sources(_metal_sources METAL_SWIFT "${_metal_dir}")
  obs_graphics_sources(_metal_bridge METAL_BRIDGE "${_metal_dir}")
  list(TRANSFORM _metal_sources PREPEND "${_metal_dir}/")
  set(_metal_output "${CMAKE_BINARY_DIR}/libobs-metal/libobs-metal.dylib")
  add_custom_command(
    OUTPUT "${_metal_output}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_BINARY_DIR}/libobs-metal"
    COMMAND
      "${OBS_CROSS_SWIFTC}" ${OBS_CROSS_SWIFT_ARGS} -swift-version 6 -import-objc-header
      "${_metal_dir}/${_metal_bridge}" -I "${OBS_SOURCE_ROOT}/libobs" -I "${CMAKE_BINARY_DIR}/config" -I
      "${OBS_CROSS_DEPS}/include" -module-name libobs_metal -parse-as-library -emit-library -Xlinker -install_name
      -Xlinker "@rpath/libobs-metal.dylib" -o "${_metal_output}" ${_metal_sources} "$<TARGET_FILE:libobs-cross-shared>"
    DEPENDS libobs-cross-shared ${_metal_sources} "${_metal_dir}/${_metal_bridge}"
    VERBATIM
  )
  add_custom_target(libobs-metal DEPENDS "${_metal_output}")
endif()
add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" "${CMAKE_BINARY_DIR}/test/cmocka")
