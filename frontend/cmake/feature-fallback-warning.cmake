include_guard(DIRECTORY)

# The shared fallback warning (obs-rust/obs-studio#2218). The decisions are
# rust/obs-fallback-warning, linked into the frontend through the frontend's
# staticlib rust/obs-frontend (frontend-rust.cmake). Always built: without
# the frontend's Rust it only logs and accepts.
if(TARGET obs_frontend-static)
  obs_frontend_add_sources(FALLBACK_WARNING_RUST_HEADER)
endif()

obs_frontend_add_sources(FALLBACK_WARNING)
