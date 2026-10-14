target_sources(
  obs-studio
  PRIVATE
    utility/CrashHandler_Linux.cpp
    utility/NativeEventFilter.cpp
    utility/platform-x11.cpp
    utility/system-info-posix.cpp
)
target_compile_definitions(
  obs-studio
  PRIVATE OBS_INSTALL_PREFIX="${OBS_INSTALL_PREFIX}" $<$<BOOL:${ENABLE_PORTABLE_CONFIG}>:ENABLE_PORTABLE_CONFIG>
)
target_link_libraries(obs-studio PRIVATE Qt::DBus)

find_package(Libpci REQUIRED)
target_link_libraries(obs-studio PRIVATE Libpci::pci)

if(TARGET OBS::python)
  find_package(Python REQUIRED COMPONENTS Interpreter Development)
  target_link_libraries(obs-studio PRIVATE Python::Python)
  target_link_options(obs-studio PRIVATE LINKER:-no-as-needed)
endif()

include("${CMAKE_CURRENT_LIST_DIR}/linux/appdata-release-date.cmake")

if(NOT DEFINED GIT_HASH)
  if(EXISTS "${CMAKE_SOURCE_DIR}/.git")
    execute_process(
      COMMAND git rev-parse HEAD
      OUTPUT_VARIABLE GIT_HASH
      WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
      OUTPUT_STRIP_TRAILING_WHITESPACE
    )
  else()
    set(GIT_HASH "master")
  endif()
endif()

# The Rust build's release is its own version, linked to its own release page (#631).
set(_obs_metainfo_version "${OBS_VERSION}")
if(ENABLE_RUST_LIBOBS)
  set(OBS_VERSION "${OBS_RUST_VERSION}")
endif()
configure_file(cmake/linux/com.obsproject.Studio.metainfo.xml.in ${OBS_DESKTOP_ID}.metainfo.xml @ONLY)
set(OBS_VERSION "${_obs_metainfo_version}")
if(ENABLE_RUST_LIBOBS)
  file(READ "${CMAKE_CURRENT_BINARY_DIR}/${OBS_DESKTOP_ID}.metainfo.xml" _obs_metainfo)
  string(
    REPLACE
    "https://github.com/obsproject/obs-studio/releases/tag/${OBS_RUST_VERSION}"
    "https://github.com/obs-rust/obs-studio/releases/tag/obs-studio-rust-v${OBS_RUST_VERSION}"
    _obs_metainfo
    "${_obs_metainfo}"
  )
  file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/${OBS_DESKTOP_ID}.metainfo.xml" "${_obs_metainfo}")
endif()

install(
  FILES "${CMAKE_CURRENT_BINARY_DIR}/${OBS_DESKTOP_ID}.metainfo.xml"
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/metainfo"
)

configure_file(cmake/linux/com.obsproject.Studio.desktop "${OBS_DESKTOP_ID}.desktop" @ONLY)
install(
  FILES "${CMAKE_CURRENT_BINARY_DIR}/${OBS_DESKTOP_ID}.desktop"
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/applications"
)

install(
  FILES cmake/linux/icons/obs-logo-128.png
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/icons/hicolor/128x128/apps"
  RENAME "${OBS_DESKTOP_ID}.png"
)

install(
  FILES cmake/linux/icons/obs-logo-256.png
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/icons/hicolor/256x256/apps"
  RENAME "${OBS_DESKTOP_ID}.png"
)

install(
  FILES cmake/linux/icons/obs-logo-512.png
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/icons/hicolor/512x512/apps"
  RENAME "${OBS_DESKTOP_ID}.png"
)

install(
  FILES cmake/linux/icons/obs-logo-scalable.svg
  DESTINATION "${CMAKE_INSTALL_DATAROOTDIR}/icons/hicolor/scalable/apps"
  RENAME "${OBS_DESKTOP_ID}.svg"
)
