find_package(LibUUID REQUIRED)
find_package(X11 REQUIRED)
find_package(X11_XCB REQUIRED)
find_package(XCB REQUIRED XCB OPTIONAL_COMPONENTS XINPUT)
find_package(Gio)

libobs_add_host_sources(LINUX)

target_compile_definitions(
  libobs-objects
  PUBLIC
    OBS_INSTALL_PREFIX="${OBS_INSTALL_PREFIX}"
    $<$<COMPILE_LANG_AND_ID:C,GNU>:ENABLE_DARRAY_TYPE_TEST>
    $<$<COMPILE_LANG_AND_ID:CXX,GNU>:ENABLE_DARRAY_TYPE_TEST>
)

if(CMAKE_C_COMPILER_ID STREQUAL GNU)
  # * Silence type-limits warning in line 292 of libobs/utils/utf8.c
  if(CMAKE_C_COMPILER_VERSION VERSION_GREATER_EQUAL 12.3.0)
    target_compile_options(libobs-objects PUBLIC -Wno-error=type-limits)
  endif()
endif()

set(CMAKE_M_LIBS "")
include(CheckCSourceCompiles)
set(LIBM_TEST_SOURCE "#include<math.h>\nfloat f; int main(){sqrt(f);return 0;}")
check_c_source_compiles("${LIBM_TEST_SOURCE}" HAVE_MATH_IN_STD_LIB)

set(UUID_TEST_SOURCE "#include<uuid/uuid.h>\nint main(){return 0;}")
check_c_source_compiles("${UUID_TEST_SOURCE}" HAVE_UUID_HEADER)

if(NOT HAVE_UUID_HEADER)
  message(FATAL_ERROR "Required system header <uuid/uuid.h> not found.")
endif()

target_link_libraries(
  libobs-objects
  PUBLIC
    X11::X11
    X11::XCB
    XCB::XCB
    LibUUID::LibUUID
    ${CMAKE_DL_LIBS}
    $<$<NOT:$<BOOL:${HAVE_MATH_IN_STD_LIB}>>:m>
    $<$<TARGET_EXISTS:XCB::XINPUT>:XCB::XINPUT>
)

if(ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)

  target_sources(
    libobs-objects
    PRIVATE
      build/libobs.pulse+.h.c
      audio-monitoring/pulse/pulseaudio-monitor-format.h
      audio-monitoring/pulse/pulseaudio-stream-flags.h
      audio-monitoring/pulse/pulseaudio-wrapper.h
  )

  # pulseaudio-stream-flags.c and pulseaudio-monitor-format.c have Rust ports: C
  # reference/rollback only (sources.cmake).
  obs_libobs_add_c_sources(${LIBOBS_PULSE_PORTED_SOURCES})
  target_link_libraries(libobs-objects PUBLIC PulseAudio::PulseAudio)
  target_enable_feature(libobs "PulseAudio audio monitoring (Linux)")
else()
  target_sources(libobs-objects PRIVATE build/libobs.null-audio+.h.c)
  target_disable_feature(libobs "PulseAudio audio monitoring (Linux)")
endif()

if(TARGET gio::gio)
  # platform-nix-dbus.c and platform-nix-portal.c have Rust ports:
  # C reference/rollback only (sources.cmake).
  obs_libobs_add_c_sources(${LIBOBS_NIX_GIO_PORTED_SOURCES})
  target_link_libraries(libobs-objects PUBLIC gio::gio)
endif()

# GIO_FOUND reaches the Rust os_inhibit_sleep_* as the libobs-rust feature
# gio (#1500): the portal and session D-Bus inhibitors are Rust (#1631, #1576).
# Without GIO, only the xdg-screensaver fallback runs, as in
# util/platform-nix-inhibit.c.
if(TARGET libobs_rust)
  corrosion_set_features(libobs_rust NO_DEFAULT_FEATURES)
  if(TARGET gio::gio)
    corrosion_set_features(libobs_rust FEATURES gio)
  endif()
endif()

if(ENABLE_WAYLAND)
  find_package(Wayland REQUIRED Client)
  find_package(Xkbcommon REQUIRED)

  target_sources(libobs-objects PRIVATE build/libobs.nix-wayland+.h.c obs-nix-hotkey-state.h)
  # obs-nix-hotkey-state.c has a Rust port: C reference/rollback only (sources.cmake).
  obs_libobs_add_c_sources(${LIBOBS_WAYLAND_PORTED_SOURCES})
  target_link_libraries(libobs-objects PUBLIC Wayland::Client xkbcommon::xkbcommon)

  if(TARGET gio::gio)
    target_sources(
      libobs-objects
      PRIVATE
        build/libobs.nix-portal-shortcut-sets+.h.c
        build/libobs.nix-portal-shortcuts+.h.c
        obs-nix-portal-shortcuts.h
    )
    # obs-nix-portal-shortcut-ids.c has a Rust port: C reference/rollback only (sources.cmake).
    obs_libobs_add_c_sources(${LIBOBS_WAYLAND_GIO_PORTED_SOURCES})
    target_compile_definitions(libobs-objects PUBLIC HAVE_GLOBAL_SHORTCUTS_PORTAL)
  endif()
  target_enable_feature(libobs "Wayland compositor support (Linux)")
else()
  target_disable_feature(libobs "Wayland compositor support (Linux)")
endif()

set_target_properties(${LIBOBS_SHARED_TARGETS} PROPERTIES OUTPUT_NAME obs)
