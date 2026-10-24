include_guard(DIRECTORY)

# GPU acceleration status light (obs-rust/obs-studio#846). The classifier is
# rust/obs-gpu-status, linked into the frontend through the staticlib
# rust/obs-frontend (like obs-nvenc links obs-nvenc-plugin), so
# libobs' exported symbols do not change.
#
# It is built whenever cargo is available (#946): the crate has no crates.io
# dependencies. A build without cargo (the Flatpak sandbox) configures without
# the light and says so.
option(ENABLE_GPU_STATUS "Build the GPU acceleration status light (needs cargo)" ON)

if(ENABLE_GPU_STATUS AND NOT COMMAND corrosion_import_crate)
  # libobs/cmake/rust.cmake fetches Corrosion whenever libobs-with-rust is
  # built; otherwise this fetches the same version.
  find_program(OBS_GPU_STATUS_CARGO cargo)
  if(OBS_GPU_STATUS_CARGO)
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

if(ENABLE_GPU_STATUS AND COMMAND corrosion_import_crate)
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
    get_target_property(_obs_gpu_status_link_dirs obs_frontend-static INTERFACE_LINK_DIRECTORIES)
    if(_obs_gpu_status_link_dirs)
      list(REMOVE_ITEM _obs_gpu_status_link_dirs "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib")
      set_property(
        TARGET obs_frontend-static
        PROPERTY INTERFACE_LINK_DIRECTORIES "${_obs_gpu_status_link_dirs}"
      )
    endif()
  endif()
  target_link_libraries(obs-studio PRIVATE obs_frontend-static)
  # A staticlib's #[link(name = ...)] system libraries don't reach the final
  # link: the Windows GPU engine counters (#1228) need pdh.lib.
  target_link_libraries(obs-studio PRIVATE $<$<PLATFORM_ID:Windows>:pdh>)
  obs_frontend_add_sources(GPU_STATUS_RUST_HEADER)
  target_enable_feature(obs-studio "GPU acceleration status light" OBS_GPU_STATUS)
elseif(ENABLE_GPU_STATUS)
  message(WARNING "cargo not found: building without the GPU acceleration status light (obs-rust/obs-studio#946)")
  target_disable_feature(obs-studio "GPU acceleration status light (cargo not found)")
else()
  target_disable_feature(obs-studio "GPU acceleration status light")
endif()

obs_frontend_add_sources(GPU_STATUS)
