# Characterization of the Windows key map and key name route of
# libobs/obs-windows.c, moved to libobs/obs-windows-keys.c and ported to
# rust/obs-hotkey (windows_keys): the virtual key table, the Meta rule of the
# pressed check, the key name route with its scan code and system name gate,
# the two virtual key lookups and the combination order. Builds the moved
# file on its own: no Windows header and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The Tier 3 parity tests in
# rust/obs-hotkey compare the shims with this C too.
# Target and test are declared in ../build/test-graph.d.

# The same cases against the Rust shims, built and run on every platform with
# the hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target).
# obs_windows_keys_* are hidden in the shared library, so the test takes them
# from the production staticlib at executable link time and compiles none of
# the C. The staticlib's obs-hotkey and obs-util members also reference libobs's
# hidden hotkey seams and logging halves, which libobs links but does not
# export; windows-keys-rust-stubs.c stands in for them (each aborts: the shims
# call none of them). Those members also need obs-util's system libraries,
# which the executable links as libobs does: jansson everywhere, GIO and libuuid
# only where libobs links them itself (the OS_LINUX and FreeBSD builds), so
# both are optional targets here. The staticlib's host halves call the platform
# directly too, so the executable carries libobs's own per-host link set: winmm
# and Rpcrt4 on Windows (timeBeginPeriod()/timeEndPeriod(), UuidCreate()), the
# frameworks of libobs/cmake/os-macos.cmake on macOS. The remaining libobs
# symbols the shims pull in (blog, bcrash, obs_get_video_info) are public and
# exported on every host, and the shims carry no host code, so one set of stubs
# serves all three.
# Both targets/tests are declared in ../build/test-graph.d. Keep the
# original optional imported-target checks, including on non-Linux hosts.
set(HAVE_WINDOWS_KEYS_TEST_UUID FALSE)
if(TARGET LibUUID::LibUUID)
  set(HAVE_WINDOWS_KEYS_TEST_UUID TRUE)
endif()
set(HAVE_WINDOWS_KEYS_TEST_GIO FALSE)
if(TARGET gio::gio)
  set(HAVE_WINDOWS_KEYS_TEST_GIO TRUE)
endif()
# Public core references arise from the Rust archive: the graph puts that
# archive before OBS::libobs so --as-needed does not discard their provider.
