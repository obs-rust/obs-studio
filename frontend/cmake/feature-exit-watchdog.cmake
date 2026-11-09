include_guard(DIRECTORY)

# The exit watchdog (obs-rust/obs-studio#4065): ends OBS when exit() blocks
# after shutdown. In rust/obs-exit-watchdog, linked into the frontend through
# the frontend's staticlib rust/obs-frontend (frontend-rust.cmake). Without
# the frontend's Rust there is no watchdog.
if(TARGET obs_frontend-static)
  obs_frontend_add_sources(EXIT_WATCHDOG_RUST_HEADER)
endif()
