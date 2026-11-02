# Characterization of the distribution, Flatpak and desktop log rules of
# libobs/obs-nix.c and obs-nix-os-release.h, moved to
# libobs/obs-nix-log-rules.c before their port to rust/obs-core
# (nix_system_info): the os-release keys, value span, quote cut and version
# fallback, saved os-release text through a copy of read_distribution_info()'s
# loop, the Flatpak extension names and lists, and the desktop line forms.
# Builds the moved file into itself and needs nothing from libobs, so it runs
# on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member also holds the
# rest of obs-core, which pulls in libobs exports and obs-util (libuuid, GIO),
# so it does not link on its own, as for test_context_name_helpers. The Tier 3
# parity tests in rust/obs-core compare the shims with this C instead, the
# intentional difference (#3473) included. Target and test are declared in
# ../build/test-graph.json.
