# Characterization of the keysym tables and the keymap and focused-key rules
# of libobs/obs-nix-wayland.c, moved to libobs/obs-nix-wayland-keys.c before
# their port: both tables over every keysym and key they name, their round
# trips, load_keymap_data()'s rule for one keycode and set_focused_key()'s
# rule. Builds the moved file on its own: no compositor and no libobs core.
# It needs only the keysym names of xkbcommon, so it is built where the
# Wayland platform is. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_wayland_volume_keys, test_wayland_keymap and test_wayland_multiseat
# cover the callers through the real platform.
set(HAVE_NIX_WAYLAND_KEYS_TEST_SDK FALSE)
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_WAYLAND)
  find_package(Xkbcommon)
  if(TARGET xkbcommon::xkbcommon)
    set(HAVE_NIX_WAYLAND_KEYS_TEST_SDK TRUE)
  endif()
endif()
# Target and test are declared in ../build/test-graph.json.
