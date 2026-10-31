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

# Whole-archive so every exported C symbol ends up in ${OBS_LIBOBS_RUST_TARGET}, including ones
# libobs itself never calls (e.g. bitstream_reader_r16, used by plugins).
target_link_libraries(${OBS_LIBOBS_RUST_TARGET} PRIVATE "$<LINK_LIBRARY:WHOLE_ARCHIVE,libobs_rust-static>")

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
