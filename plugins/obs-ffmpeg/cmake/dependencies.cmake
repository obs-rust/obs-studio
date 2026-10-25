find_package(
  FFmpeg
  6.1
  REQUIRED avcodec avfilter avdevice avutil swscale avformat swresample
)

if(NOT TARGET OBS::media-playback)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/media-playback" "${CMAKE_BINARY_DIR}/shared/media-playback")
endif()

if(NOT TARGET OBS::opts-parser)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/opts-parser" "${CMAKE_BINARY_DIR}/shared/opts-parser")
endif()

if(OS_WINDOWS AND CMAKE_VS_PLATFORM_NAME STREQUAL x64)
  find_package(AMF 1.4.29 REQUIRED)
  add_subdirectory(obs-amf-test)
elseif(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(Libva REQUIRED)
  find_package(Libpci REQUIRED)
  find_package(Libdrm REQUIRED)
endif()

if(ENABLE_NEW_MPEGTS_OUTPUT)
  find_package(Librist QUIET)
  find_package(Libsrt QUIET)

  foreach(_output_lib IN ITEMS Librist Libsrt)
    if(NOT TARGET ${_output_lib}::${_output_lib})
      list(APPEND _error_messages "MPEGTS output library ${_output_lib} not found.")
    endif()
  endforeach()

  # The packages OBS builds with its own libsrt (Windows and macOS from
  # obs-deps, the Flatpak from obs-deps-buildstream) need at least 1.5.7: older
  # releases have public advisories (GHSA-6xg9-784j-24rm, GHSA-4mc6-qmpp-g7gw,
  # GHSA-m96h-3rc9-xx6h; obs-rust/obs-studio#2372). The AppImage checks its
  # libsrt in build-aux/appimage/check-appdir.sh. Distribution packages use the
  # distribution's libsrt and are not checked here.
  # An unknown version (0.0.0) fails too: the check never passes a version it
  # cannot read.
  set(_libsrt_minimum 1.5.7)
  if((OS_WINDOWS OR OS_MACOS OR OS_FLATPAK) AND TARGET Libsrt::Libsrt)
    if(Libsrt_VERSION VERSION_EQUAL 0.0.0)
      list(
        APPEND _error_messages
        "Cannot read the Libsrt version: at least ${_libsrt_minimum} is required (obs-rust/obs-studio#2372)."
      )
    elseif(Libsrt_VERSION VERSION_LESS _libsrt_minimum)
      list(
        APPEND _error_messages
        "Libsrt ${Libsrt_VERSION} is too old: at least ${_libsrt_minimum} is required (public advisories, see obs-rust/obs-studio#2372)."
      )
    endif()
  endif()

  if(_error_messages)
    list(JOIN _error_messages "\n" _error_string)
    message(
      FATAL_ERROR
      "${_error_string}\n Disable this error by setting ENABLE_NEW_MPEGTS_OUTPUT to OFF or providing the build system with required SRT and Rist libraries."
    )
  endif()
endif()
