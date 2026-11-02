# Characterization of the distribution, Flatpak and desktop log rules of
# libobs/obs-nix.c and obs-nix-os-release.h, moved to
# libobs/obs-nix-log-rules.c before their port to rust/obs-core
# (nix_system_info): the os-release keys, value span, quote cut and version
# fallback, saved os-release text through a copy of read_distribution_info()'s
# loop, the Flatpak extension names and lists, and the desktop line forms.
# Builds the moved file into itself and needs nothing from libobs, so it runs
# on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. Target and test are
# declared in ../build/test-graph.json.
