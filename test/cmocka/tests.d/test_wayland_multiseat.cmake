# Wayland hotkeys with several seats (obsproject/obs-studio#12929). Drives
# the real Wayland hotkey platform against a fake compositor that runs in a
# child process over a socketpair; no display is needed.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_WAYLAND)
  find_package(Wayland COMPONENTS Client Server)
  find_package(Xkbcommon)
  if(TARGET Wayland::Server AND TARGET xkbcommon::xkbcommon)
    add_executable(
      test_wayland_multiseat
      test_wayland_multiseat.c
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-wayland.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-hotkey-state.c.h"
      hotkey_mutex_stub.c
    )
    target_compile_definitions(test_wayland_multiseat PRIVATE obs=wayland_test_obs)
    target_include_directories(test_wayland_multiseat PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
    target_link_libraries(
      test_wayland_multiseat
      PRIVATE OBS::libobs OBS::caption Wayland::Client Wayland::Server xkbcommon::xkbcommon ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_wayland_multiseat)
    add_test(test_wayland_multiseat ${CMAKE_CURRENT_BINARY_DIR}/test_wayland_multiseat)
  endif()
endif()
