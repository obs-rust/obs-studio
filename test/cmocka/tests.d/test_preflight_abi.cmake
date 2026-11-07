# The unchanged ABI executable is registered by the shared native graph.
set(HAVE_PREFLIGHT_TEST_ARCHIVE FALSE)
if(OS_LINUX OR OS_MACOS)
  find_program(SOLDR_EXECUTABLE soldr)
  if(NOT SOLDR_EXECUTABLE)
    return()
  endif()
  set(_preflight_cargo_command "${SOLDR_EXECUTABLE}" cargo)
  set(_preflight_cargo_environment)
  set(_preflight_manifest "${CMAKE_SOURCE_DIR}/rust/obs-preflight-ffi/Cargo.toml")
  set(_preflight_target_dir "${CMAKE_BINARY_DIR}/obs-preflight-cargo")
  list(APPEND _preflight_cargo_environment "CARGO_TARGET_DIR=${_preflight_target_dir}")
  file(
    GLOB_RECURSE _preflight_rust_sources
    CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/rust/obs-preflight/src/*.rs"
    "${CMAKE_SOURCE_DIR}/rust/obs-preflight-ffi/src/*.rs"
  )
  set(
    _preflight_archive
    "${_preflight_target_dir}/release/${CMAKE_STATIC_LIBRARY_PREFIX}obs_preflight_ffi${CMAKE_STATIC_LIBRARY_SUFFIX}"
  )
  add_custom_command(
    OUTPUT "${_preflight_archive}"
    COMMAND
      "${CMAKE_COMMAND}" -E env ${_preflight_cargo_environment} ${_preflight_cargo_command} build --manifest-path
      "${CMAKE_SOURCE_DIR}/Cargo.toml" --package obs-preflight-ffi --lib --release --locked
    DEPENDS
      "${_preflight_manifest}"
      "${CMAKE_SOURCE_DIR}/rust/obs-preflight/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/Cargo.lock"
      ${_preflight_rust_sources}
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    COMMENT "Building obs-preflight Rust C ABI"
    VERBATIM
  )
  add_custom_target(obs-preflight-rust DEPENDS "${_preflight_archive}")

  # Corrosion builds each imported crate with a separate Cargo command. Keep
  # this invocation behind libobs's Cargo target so Rustup cannot install the
  # pinned toolchain's components concurrently in both commands.
  if(TARGET libobs_rust-static)
    if(TARGET cargo-build_libobs_rust)
      add_dependencies(obs-preflight-rust cargo-build_libobs_rust)
    else()
      message(FATAL_ERROR "Corrosion's cargo-build_libobs_rust target was not found")
    endif()
  endif()

  add_library(OBS::preflight-test-archive INTERFACE IMPORTED)
  set_target_properties(OBS::preflight-test-archive PROPERTIES INTERFACE_LINK_LIBRARIES "${_preflight_archive}")
  add_dependencies(OBS::preflight-test-archive obs-preflight-rust)
  set(HAVE_PREFLIGHT_TEST_ARCHIVE TRUE)
endif()
