include_guard(DIRECTORY)

# GPU acceleration status light (obs-rust/obs-studio#846). The classifier is
# rust/obs-gpu-status, linked into the frontend through the frontend's
# staticlib rust/obs-frontend (frontend-rust.cmake), so libobs' exported
# symbols do not change.
#
# It is built whenever the frontend's Rust is (#946). The Flatpak turns the
# light off.
option(ENABLE_GPU_STATUS "Build the GPU acceleration status light (needs cargo)" ON)

if(ENABLE_GPU_STATUS AND TARGET obs_frontend-static)
  obs_frontend_add_sources(GPU_STATUS_RUST_HEADER)
  target_enable_feature(obs-studio "GPU acceleration status light" OBS_GPU_STATUS)
elseif(ENABLE_GPU_STATUS)
  message(WARNING "cargo not found: building without the GPU acceleration status light (obs-rust/obs-studio#946)")
  target_disable_feature(obs-studio "GPU acceleration status light (cargo not found)")
else()
  target_disable_feature(obs-studio "GPU acceleration status light")
endif()

obs_frontend_add_sources(GPU_STATUS)
