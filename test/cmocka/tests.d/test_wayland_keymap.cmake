# A keymap the Wayland hotkey platform can't use keeps the last good one
# (obs-rust/obs-studio#2975). Compiles in the real Wayland hotkey platform;
# no display is needed.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_WAYLAND)
  find_package(Wayland COMPONENTS Client)
  find_package(Xkbcommon)
  if(TARGET Wayland::Client AND TARGET xkbcommon::xkbcommon)
    add_executable(
      test_wayland_keymap
      test_wayland_keymap.c
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-hotkey-state.c.h"
      hotkey_mutex_stub.c
    )
    target_compile_definitions(test_wayland_keymap PRIVATE obs=wayland_test_obs)
    target_include_directories(test_wayland_keymap PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
    target_link_libraries(
      test_wayland_keymap
      PRIVATE OBS::libobs OBS::caption Wayland::Client xkbcommon::xkbcommon ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_wayland_keymap)
    add_test(test_wayland_keymap ${CMAKE_CURRENT_BINARY_DIR}/test_wayland_keymap)
  endif()
endif()
