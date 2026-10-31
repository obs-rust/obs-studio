# Characterization of the window helpers' pure decisions of
# libobs/util/windows/window-helpers.c, moved to
# libobs/util/windows/window-match.c and ported to rust/obs-util
# (window_match): the internal exes and the untitled Explorer the window
# lists hide, the generic and UWP class tests, and the rating
# ms_find_window() picks a window by. Builds the moved file on its own: no
# Windows header, and libobs only for astrcmpi(), astrcmpi_n() and astrstri(),
# so it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member also holds
# the rest of obs-util, which pulls in libuuid, GIO and libobs's hidden
# base_default_log_handler, so it does not link on its own, as
# test_context_name_helpers. The Tier 3 parity tests in rust/obs-util compare
# the shims with this C instead.
# Target and test are declared in ../build/test-graph.json.
