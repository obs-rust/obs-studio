include_guard(DIRECTORY)

# The exit watchdog (obs-rust/obs-studio#4065): ends OBS when exit() blocks
# after shutdown. In rust/obs-exit-watchdog, linked into the frontend through
# the frontend's staticlib rust/obs-frontend (frontend-rust.cmake). Without
# the frontend's Rust there is no watchdog.
if(TARGET obs_frontend-static)
  obs_frontend_add_sources(EXIT_WATCHDOG_RUST_HEADER)
endif()

# Windows: the watchdog's helper process (obs-rust/obs-studio#4589), the
# obs-exit-watchdog binary of the same crate. A plugin DLL's detach runs
# inside ExitProcess, after the watchdog thread has ended, so only another
# process can end OBS when it blocks there. The frontend starts the helper
# right after it opens its log file; it looks for it next to obs64.exe.
# The release signing (ci/windows_sign.py) signs every .exe in the package.
if(OS_WINDOWS AND TARGET obs_frontend-static)
  corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
    CRATES obs-exit-watchdog
    PROFILE release
  )
  # A binary's Corrosion targets keep its hyphens (cargo-build_obs-exit-watchdog),
  # so obs_serialize_cargo_build(), which expects a library's underscores,
  # is spelled out here: one cargo build at a time.
  get_property(_obs_previous_cargo_build GLOBAL PROPERTY OBS_PREVIOUS_CARGO_BUILD)
  if(_obs_previous_cargo_build)
    add_dependencies(cargo-prebuild_obs-exit-watchdog ${_obs_previous_cargo_build})
  endif()
  set_property(GLOBAL PROPERTY OBS_PREVIOUS_CARGO_BUILD cargo-build_obs-exit-watchdog)
  add_dependencies(obs-studio cargo-build_obs-exit-watchdog)
  install(PROGRAMS "$<TARGET_FILE:obs-exit-watchdog>" DESTINATION "${OBS_EXECUTABLE_DESTINATION}" COMPONENT Runtime)
  add_custom_command(
    TARGET obs-studio
    POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${OBS_OUTPUT_DIR}/$<CONFIG>/${OBS_EXECUTABLE_DESTINATION}"
    COMMAND
      "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:obs-exit-watchdog>"
      "${OBS_OUTPUT_DIR}/$<CONFIG>/${OBS_EXECUTABLE_DESTINATION}"
    COMMENT "Copy obs-exit-watchdog to the rundir"
    VERBATIM
  )
endif()
