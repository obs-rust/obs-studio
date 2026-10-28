# The Wayland key table names Volume Up and Down the right way round
# (obs-rust/obs-studio#2635). Compiles in the real Wayland hotkey platform;
# no display is needed.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_WAYLAND)
  find_package(Wayland COMPONENTS Client)
  find_package(Xkbcommon)
  if(TARGET Wayland::Client AND TARGET xkbcommon::xkbcommon)
    add_executable(
      test_wayland_volume_keys
      test_wayland_volume_keys.c
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-wayland.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-hotkey-state.c.h"
      hotkey_mutex_stub.c
    )
    target_compile_definitions(test_wayland_volume_keys PRIVATE obs=wayland_test_obs)
    target_include_directories(test_wayland_volume_keys PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
    target_link_libraries(
      test_wayland_volume_keys
      PRIVATE OBS::libobs OBS::caption Wayland::Client xkbcommon::xkbcommon ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_wayland_volume_keys)
    add_test(test_wayland_volume_keys ${CMAKE_CURRENT_BINARY_DIR}/test_wayland_volume_keys)
  endif()
endif()
