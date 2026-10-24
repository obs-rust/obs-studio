include_guard(DIRECTORY)

# The frontend's Rust (obs-rust/obs-studio#2218): one staticlib,
# rust/obs-frontend, with the C ABI of the frontend's crates (obs-gpu-status
# #846, obs-fallback-warning #2218). One staticlib per binary: two would
# duplicate the Rust std symbols. Linked like obs-nvenc links
# obs-nvenc-plugin, so libobs' exported symbols do not change.
#
# Built whenever cargo is available. The default libobs backend (HYBRID)
# needs cargo already. Without it (the OBS_LIBOBS_BACKEND=C rollback build)
# the frontend configures without its Rust and says so: the GPU light is off,
# and the fallback warning only logs and accepts.
if(NOT COMMAND corrosion_import_crate)
  # libobs/cmake/rust.cmake fetches Corrosion whenever libobs-with-rust is
  # built; otherwise this fetches the same version.
  find_program(OBS_FRONTEND_CARGO cargo)
  if(OBS_FRONTEND_CARGO)
    include(FetchContent)
    include("${CMAKE_SOURCE_DIR}/libobs/cmake/rust-target.cmake")
    if(NOT Rust_CARGO_TARGET)
      obs_rust_cargo_target(_obs_rust_target "${OS_MACOS}" "${CMAKE_OSX_ARCHITECTURES}")
      if(_obs_rust_target)
        set(Rust_CARGO_TARGET "${_obs_rust_target}" CACHE STRING "Rust target triple for the frontend's Rust code")
      endif()
    endif()
    FetchContent_Declare(Corrosion GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git GIT_TAG v0.5.2)
    FetchContent_MakeAvailable(Corrosion)
  endif()
endif()

if(COMMAND corrosion_import_crate)
  corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
    CRATES obs-frontend
    CRATE_TYPES staticlib
    PROFILE release
  )
  obs_serialize_cargo_build(obs-frontend)
  if(OS_MACOS)
    # Same as libobs/cmake/rust.cmake (#777): let the linker use the selected
    # Xcode SDK, not the CLT SDK Corrosion injects.
    get_target_property(_obs_frontend_link_dirs obs_frontend-static INTERFACE_LINK_DIRECTORIES)
    if(_obs_frontend_link_dirs)
      list(REMOVE_ITEM _obs_frontend_link_dirs "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib")
      set_property(TARGET obs_frontend-static PROPERTY INTERFACE_LINK_DIRECTORIES "${_obs_frontend_link_dirs}")
    endif()
  endif()
  target_link_libraries(obs-studio PRIVATE obs_frontend-static)
  # A staticlib's #[link(name = ...)] system libraries don't reach the final
  # link: the Windows GPU engine counters (#1228) need pdh.lib.
  target_link_libraries(obs-studio PRIVATE $<$<PLATFORM_ID:Windows>:pdh>)
  target_enable_feature(obs-studio "Frontend Rust (rust/obs-frontend)" OBS_FRONTEND_RUST)
else()
  message(
    WARNING
    "cargo not found: building the frontend without its Rust (rust/obs-frontend, obs-rust/obs-studio#2218)"
  )
  target_disable_feature(obs-studio "Frontend Rust (cargo not found)")
endif()
