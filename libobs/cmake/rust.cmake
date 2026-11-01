# Link completed Rust ports into the production hybrid libobs. The C reference
# never includes this archive; the public C ABI and export set stay identical.

include(FetchContent)
include("${CMAKE_CURRENT_LIST_DIR}/rust-target.cmake")

# Build the staticlib for the architecture libobs targets, not the host's.
if(NOT Rust_CARGO_TARGET)
  obs_rust_cargo_target(_obs_rust_target "${OS_MACOS}" "${CMAKE_OSX_ARCHITECTURES}")
  if(_obs_rust_target)
    set(Rust_CARGO_TARGET "${_obs_rust_target}" CACHE STRING "Rust target triple for libobs-rust")
  endif()
endif()

FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.5.2)
FetchContent_MakeAvailable(Corrosion)

# Pin the release profile so the staticlib always builds with
# panic=abort (set only for [profile.release] in Cargo.toml): without
# PROFILE, Corrosion picks dev for Debug or unset CMAKE_BUILD_TYPE, and a
# panic in a no_mangle shim would then unwind across the C ABI (UB).
corrosion_import_crate(
  MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
  CRATES libobs-rust
  CRATE_TYPES staticlib
  PROFILE release
)
obs_serialize_cargo_build(libobs-rust)

# Cross-language ThinLTO (#3217), applied by the ELF host file
# (rust-os-nix.cmake); declared here so the module-cache key below sees it.
option(OBS_CROSS_LTO "Cross-language ThinLTO between libobs C and the Rust staticlib (Clang, lld)" OFF)
mark_as_advanced(OBS_CROSS_LTO)

# The module cache (#3265): ci/libobs_modules.py keys the staticlib on what
# this file records, plus Cargo.lock, the toolchain and the linked crates.
set(_obs_rust_features "")
if(TARGET libobs_rust)
  foreach(_name IN ITEMS FEATURES NO_DEFAULT_FEATURES ALL_FEATURES)
    string(TOLOWER "${_name}" _key)
    string(APPEND _obs_rust_features ", \"${_key}\": \"$<TARGET_PROPERTY:libobs_rust,INTERFACE_CORROSION_${_name}>\"")
  endforeach()
endif()
file(
  GENERATE OUTPUT "${CMAKE_BINARY_DIR}/libobs-modcache-rust.json"
  CONTENT
    "{\"location\": \"$<TARGET_FILE:libobs_rust-static>\"${_obs_rust_features}, \"target\": \"${Rust_CARGO_TARGET}\", \"profile\": \"release\", \"cc\": \"${CMAKE_C_COMPILER}\", \"cross_lto\": \"${OBS_CROSS_LTO}\"}\n"
)

# A hit links the cached archive; cargo does not run. Corrosion still
# configures the crate, so the archive keeps its native link libraries.
set(_obs_rust_static libobs_rust-static)
if(OBS_LIBOBS_MODULE_CACHE AND EXISTS "${OBS_LIBOBS_MODULE_CACHE}/hits.cmake")
  include("${OBS_LIBOBS_MODULE_CACHE}/hits.cmake")
endif()
if(LIBOBS_RUST_HIT)
  add_library(libobs_rust-cached STATIC IMPORTED GLOBAL)
  set_property(TARGET libobs_rust-cached PROPERTY IMPORTED_LOCATION "${LIBOBS_RUST_HIT}")
  set_property(TARGET cargo-build_libobs_rust PROPERTY EXCLUDE_FROM_ALL TRUE)
  set(_obs_rust_static libobs_rust-cached)
  message(STATUS "libobs-rust: restored from the module cache")
endif()

# Whole-archive so every exported C symbol ends up in ${OBS_LIBOBS_RUST_TARGET}, including ones
# libobs itself never calls (e.g. bitstream_reader_r16, used by plugins).
target_link_libraries(${OBS_LIBOBS_RUST_TARGET} PRIVATE "$<LINK_LIBRARY:WHOLE_ARCHIVE,${_obs_rust_static}>")

# Keep ${OBS_LIBOBS_RUST_TARGET} exporting the same symbol set as libobs:
# - ELF: Rust staticlib internals have default visibility; hide them with a
#   version script.
# - Windows: Rust no_mangle symbols in a staticlib are not dllexport, so export
#   the listed shims explicitly (otherwise its obs.dll and import lib miss them).
# - macOS: hide Rust internals with ld64 -unexported_symbols_list.
# The ELF and macOS lists are generated from rust-hidden-base.txt and one
# rust-hidden.d/ fragment per FFI module (#3233), hiding the HEVC shims too
# when ENABLE_HEVC is off: the aggregate archive contains them either way, and
# the public surface must match that configuration's C baseline. Each host
# file applies its list.
include("${CMAKE_CURRENT_LIST_DIR}/rust-hidden.cmake")
obs_rust_hidden_lists(
  "${CMAKE_CURRENT_LIST_DIR}/rust-hidden.d"
  "${CMAKE_CURRENT_LIST_DIR}/rust-hidden-base.txt"
  "${CMAKE_CURRENT_BINARY_DIR}"
  "${ENABLE_HEVC}"
)
set(_obs_rust_export_map "${CMAKE_CURRENT_BINARY_DIR}/rust-exports.map")
set(_obs_rust_unexports "${CMAKE_CURRENT_BINARY_DIR}/rust-unexports-macos.txt")
if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  include("${CMAKE_CURRENT_LIST_DIR}/rust-os-nix.cmake")
elseif(OS_WINDOWS)
  include("${CMAKE_CURRENT_LIST_DIR}/rust-os-windows.cmake")
elseif(OS_MACOS)
  include("${CMAKE_CURRENT_LIST_DIR}/rust-os-macos.cmake")
endif()

# The cached archive links like Corrosion's, including what the host files
# above set on it.
if(LIBOBS_RUST_HIT)
  foreach(_property IN ITEMS INTERFACE_LINK_LIBRARIES INTERFACE_LINK_OPTIONS INTERFACE_LINK_DIRECTORIES)
    get_target_property(_value libobs_rust-static ${_property})
    if(_value)
      set_property(TARGET libobs_rust-cached PROPERTY ${_property} "${_value}")
    endif()
  endforeach()
endif()
