include_guard(DIRECTORY)

# The dock layout (obs-rust/obs-studio#5230). The decisions are
# rust/obs-dock-layout, linked into the frontend through the frontend's
# staticlib rust/obs-frontend (frontend-rust.cmake). Always built: without
# the frontend's Rust it saves the live layout, as upstream OBS does.
if(TARGET obs_frontend-static)
  obs_frontend_add_sources(DOCK_LAYOUT_RUST_HEADER)
endif()

obs_frontend_add_sources(DOCK_LAYOUT)
