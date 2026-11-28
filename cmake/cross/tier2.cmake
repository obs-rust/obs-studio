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
endif()
find_package(
  FFmpeg
  8.0
  REQUIRED
  COMPONENTS avcodec avdevice avformat avutil swscale swresample
)
find_package(jansson REQUIRED)
find_package(Uthash REQUIRED)
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
  set_target_properties(${target} PROPERTIES ${ARGN})
endfunction()
add_subdirectory("${OBS_SOURCE_ROOT}/shared/opts-parser" "${CMAKE_BINARY_DIR}/shared/opts-parser")
add_subdirectory("${OBS_SOURCE_ROOT}/shared/bpm" "${CMAKE_BINARY_DIR}/shared/bpm")
foreach(_plugin IN ITEMS image-source obs-transitions text-freetype2 obs-filters obs-outputs)
  add_subdirectory("${OBS_SOURCE_ROOT}/plugins/${_plugin}" "${CMAKE_BINARY_DIR}/plugins/${_plugin}" EXCLUDE_FROM_ALL)
endforeach()
add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" "${CMAKE_BINARY_DIR}/test/cmocka")
