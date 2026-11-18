# Opt-in producer probe: canonical C source sets and a small unchanged registry.
# Full native registry/renderer coverage is required before any workflow switch.
# CMake's Windows-GNU-clang link rule uses -nostdlib. Ask clang to resolve the
# runtime selected by --rtlib/-resource-dir, then explicitly link it to the C
# reference. Rust-static consumers already carry Rust's compiler-builtins;
# auto-linking a second runtime there would duplicate __compilerrt_abort_impl.
execute_process(
  COMMAND "${_tier2_cc}" "--config=${_tier2_config}" -print-libgcc-file-name
  RESULT_VARIABLE _builtins_result
  OUTPUT_VARIABLE _tier2_builtins
  OUTPUT_STRIP_TRAILING_WHITESPACE
)
if(NOT _builtins_result EQUAL 0 OR NOT EXISTS "${_tier2_builtins}")
  message(FATAL_ERROR "clang did not resolve the provisioned Windows compiler-rt")
endif()
add_library(libobs-cross-reference SHARED)
target_link_libraries(
  libobs-cross-reference
  PRIVATE
    libobs-cross
    libobs-cross-obfuscate
    libobs-cross-generated
    libobs-cross-shared-pthreads
    OBS::caption
    avcodec
    avformat
    avutil
    swscale
    swresample
    jansson
    zlib
    avrt
    dwmapi
    dxgi
    winmm
    rpcrt4
    ws2_32
    ntdll
    bcrypt
    userenv
    "${_tier2_builtins}"
)
target_link_libraries(libobs-cross-shared PRIVATE zlib)
target_link_directories(libobs-cross-reference PRIVATE "${OBS_CROSS_DEPS}/lib" "${CMAKE_BINARY_DIR}/cross-case-libs")
foreach(_variant IN ITEMS shared reference)
  set_target_properties(
    libobs-cross-${_variant}
    PROPERTIES
      PREFIX ""
      OUTPUT_NAME obs
      IMPORT_PREFIX ""
      IMPORT_SUFFIX ".lib"
      RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/cores/${_variant}"
      ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/cores/${_variant}"
  )
endforeach()

# Use the actual Windows version-resource template; don't generate a test-only RC.
enable_language(RC)
function(_obs_windows_tier2_resource)
  set(CMAKE_CURRENT_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(CMAKE_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  include(bootstrap)
  configure_file("${OBS_SOURCE_ROOT}/libobs/cmake/windows/obs-module.rc.in" "${CMAKE_BINARY_DIR}/config/libobs.rc")
endfunction()
_obs_windows_tier2_resource()
target_sources(libobs-cross-generated PRIVATE "${CMAKE_BINARY_DIR}/config/libobs.rc")

# Reuse the Darwin prototype's checksum-pinned cmocka preparation.
include(FetchContent)
set(WITH_EXAMPLES OFF CACHE BOOL "" FORCE)
set(UNIT_TESTING OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS ON CACHE BOOL "Build native Windows cmocka DLL" FORCE)
FetchContent_Declare(
  cmocka
  URL https://cmocka.org/files/1.1/cmocka-1.1.7.tar.xz
  URL_HASH SHA256=810570eb0b8d64804331f82b29ff47c790ce9cd6b163e98d47a4807047ecad82
  DOWNLOAD_EXTRACT_TIMESTAMP TRUE
)
# Function-existence checks need a real link. The compile-only toolchain's
# STATIC_LIBRARY probes otherwise claim Windows has POSIX siglongjmp.
set(_tier2_try_compile "${CMAKE_TRY_COMPILE_TARGET_TYPE}")
set(_tier2_try_configuration "${CMAKE_TRY_COMPILE_CONFIGURATION}")
set(_tier2_required_options "${CMAKE_REQUIRED_LINK_OPTIONS}")
set(CMAKE_TRY_COMPILE_TARGET_TYPE EXECUTABLE)
# xwin ships the release CRT, so feature probes must use the native profile.
set(CMAKE_TRY_COMPILE_CONFIGURATION RelWithDebInfo)
list(APPEND CMAKE_REQUIRED_LINK_OPTIONS "-L${CMAKE_BINARY_DIR}/cross-case-libs")
FetchContent_MakeAvailable(cmocka)
set(CMAKE_REQUIRED_LINK_OPTIONS "${_tier2_required_options}")
set(CMAKE_TRY_COMPILE_TARGET_TYPE "${_tier2_try_compile}")
set(CMAKE_TRY_COMPILE_CONFIGURATION "${_tier2_try_configuration}")
target_link_directories(cmocka PRIVATE "${CMAKE_BINARY_DIR}/cross-case-libs")
add_library(obs-native-test-cmocka INTERFACE)
add_library(OBS::native-test-cmocka ALIAS obs-native-test-cmocka)
target_include_directories(obs-native-test-cmocka INTERFACE "${cmocka_SOURCE_DIR}/include")
target_link_libraries(obs-native-test-cmocka INTERFACE cmocka)

add_library(OBS::libobs ALIAS libobs-cross-shared)
add_library(OBS::w32-pthreads ALIAS libobs-cross-shared-pthreads)
target_include_directories(
  libobs-cross-shared
  INTERFACE "${OBS_SOURCE_ROOT}/libobs" "${CMAKE_BINARY_DIR}/config" "${OBS_SOURCE_ROOT}/deps/w32-pthreads"
)
add_library(libobs_rust-static STATIC IMPORTED GLOBAL)
set_target_properties(
  libobs_rust-static
  PROPERTIES IMPORTED_LOCATION "${OBS_SOURCE_ROOT}/target/x86_64-pc-windows-msvc/release/libobs_rust.lib"
)
add_dependencies(libobs_rust-static libobs-cross-shared-rust)
target_link_directories(libobs_rust-static INTERFACE "${CMAKE_BINARY_DIR}/cross-case-libs")
target_link_libraries(libobs_rust-static INTERFACE ws2_32 ntdll bcrypt userenv)
foreach(_component IN ITEMS avcodec avformat avutil)
  add_library(FFmpeg::${_component} UNKNOWN IMPORTED GLOBAL)
  set_target_properties(
    FFmpeg::${_component}
    PROPERTIES
      IMPORTED_LOCATION "${OBS_CROSS_DEPS}/lib/${_component}.lib"
      INTERFACE_INCLUDE_DIRECTORIES "${OBS_CROSS_DEPS}/include"
  )
endforeach()
list(PREPEND CMAKE_MODULE_PATH "${OBS_SOURCE_ROOT}/cmake/finders")
list(PREPEND CMAKE_PREFIX_PATH "${OBS_CROSS_DEPS}")
find_package(Uthash REQUIRED)
find_package(SIMDe REQUIRED)

# Consume complete entries from the canonical registry, never copied recipes.
# The explicit probe list is not a claim of native Tier 2 completeness.
set(_probe_tests test_bitstream test_darray test_serializer test_async_conversion_plan_rust)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${OBS_SOURCE_ROOT}/test/cmocka/build/test-graph.json")
file(READ "${OBS_SOURCE_ROOT}/test/cmocka/build/test-graph.json" _graph)
set(_probe "{\"schema\":1,\"targets\":{},\"tests\":{}}")
foreach(_test IN LISTS _probe_tests)
  foreach(_field IN ITEMS targets tests)
    string(JSON _entry GET "${_graph}" ${_field} ${_test})
    string(JSON _probe SET "${_probe}" ${_field} ${_test} "${_entry}")
  endforeach()
endforeach()
file(WRITE "${CMAKE_BINARY_DIR}/tier2-probe-graph.json" "${_probe}\n")
set(OS_WINDOWS TRUE)
set(OS_MACOS FALSE)
set(OS_LINUX FALSE)
set(OS_FREEBSD FALSE)
set(HAVE_LIBOBS_RUST_STATIC TRUE)
include("${OBS_SOURCE_ROOT}/test/cmocka/native-test-graph.cmake")
enable_testing()
function(_obs_windows_tier2_registry)
  # The canonical registry uses target facts, as obs-build does for *-msvc.
  # Keep CMake's actual GNU frontend untouched outside this registry scope.
  set(MSVC TRUE)
  obs_native_test_graph("${CMAKE_BINARY_DIR}/tier2-probe-graph.json" "${OBS_SOURCE_ROOT}")
endfunction()
_obs_windows_tier2_registry()
add_custom_target(tier2-cross-probe DEPENDS libobs-cross-shared libobs-cross-reference ${_probe_tests})
set(_runtime "$<TARGET_FILE:libobs-cross-shared>\n$<TARGET_FILE:libobs-cross-reference>\n")
foreach(_test IN LISTS _probe_tests)
  string(APPEND _runtime "$<TARGET_FILE:${_test}>\n")
endforeach()
file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/tier2-runtime.txt" CONTENT "${_runtime}")

file(
  WRITE "${CMAKE_BINARY_DIR}/tier2-probe-toolchain.txt"
  "registry_msvc=TRUE\ncompiler_msvc=${MSVC}\nfrontend=${CMAKE_C_COMPILER_FRONTEND_VARIANT}\ncompiler=${CMAKE_C_COMPILER}\nlinker=${CMAKE_LINKER}\nRC=${CMAKE_RC_COMPILER}\nprofile=${CMAKE_BUILD_TYPE}\n"
)
