include_guard(DIRECTORY)

# The camera-busy warning (obs-rust/obs-studio#3724). The decisions are
# rust/obs-camera-busy, linked into the frontend through the frontend's
# staticlib rust/obs-frontend (frontend-rust.cmake). Always built: without
# the frontend's Rust it does nothing and OBSBasic keeps its own popup.
if(TARGET obs_frontend-static)
  obs_frontend_add_sources(CAMERA_BUSY_RUST_HEADER)
endif()

obs_frontend_add_sources(CAMERA_BUSY)
