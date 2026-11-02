# Characterization of the system info log rules of libobs/obs-windows.c,
# moved to libobs/obs-windows-system-info.c before their port to
# rust/obs-core (win_system_info): the packed version, the memory sizes,
# the gaming feature gate, the second Game Mode read, each feature's line and
# the security product labels. Builds the moved file into itself (it takes
# no Windows type), so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.json.

# No _rust variant, as for test_win_crash_report_steps and
# test_canvas_rules_steps: the shims' archive member (an obs_core codegen
# unit) also holds the other obs-core ports, which pull in obs-util's
# platform code. On its own the archive does not link, and beside libobs it
# still needs libuuid, GIO and GLib on the link line one after another
# (checked on Linux), and would carry a second copy of libobs's Rust code.
# The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/win_system_info_parity.rs) compare the shims
# with this C instead, function by function and put together as
# obs-windows.c puts them.
