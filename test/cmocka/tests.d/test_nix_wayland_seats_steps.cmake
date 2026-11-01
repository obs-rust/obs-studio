# Characterization of the seat decisions of libobs/obs-nix-wayland.c, moved
# to libobs/obs-nix-wayland-seats.c before their port: the wl_seat bind
# version, the seat the keyboard is taken from, when a capability change
# releases it, the seat a removed global names and the release or destroy
# of a seat. Builds the moved file on its own: no Wayland header, no
# compositor and no libobs core, so it runs on every platform. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them. test_wayland_multiseat covers the callers through the real
# platform.
# Target and test are declared in ../build/test-graph.json.
