include_guard(GLOBAL)

set(OBS_DESKTOP_ID com.obsproject.Studio)
set(OBS_MACOS_BUNDLE_ID com.obsproject.obs-studio)
set(OBS_MACOS_BUNDLE_NAME OBS)
set(OBS_UNIX_GUI_NAME obs)
set(OBS_APP_DISPLAY_NAME "OBS Studio")

if(ENABLE_RUST_LIBOBS)
  set(OBS_DESKTOP_ID io.github.obs_rust.OBSStudioRust)
  set(OBS_MACOS_BUNDLE_ID io.github.obs_rust.OBSStudioRust)
  set(OBS_MACOS_BUNDLE_NAME OBS-Studio-Rust)
  # obs-studio-rust is already the websocket CLI in this repository.
  set(OBS_UNIX_GUI_NAME obs-studio-rust-gui)
  set(OBS_APP_DISPLAY_NAME OBS-Studio-Rust)
endif()

set(OBS_CAMERA_EXTENSION_ID "${OBS_MACOS_BUNDLE_ID}.mac-camera-extension")
