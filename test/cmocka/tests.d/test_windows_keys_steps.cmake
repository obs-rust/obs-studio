# Characterization of the Windows key map and key name route of
# libobs/obs-windows.c, moved to libobs/obs-windows-keys.c and ported to
# rust/obs-hotkey (windows_keys): the virtual key table, the Meta rule of the
# pressed check, the key name route with its scan code and system name gate,
# the two virtual key lookups and the combination order. Builds the moved
# file on its own: no Windows header and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The Tier 3 parity tests in
# rust/obs-hotkey compare the shims with this C too.
# Target and test are declared in ../build/test-graph.json.

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
if(TARGET libobs_rust-static)
  add_executable(test_windows_keys_steps_rust test_windows_keys_steps.c windows-keys-rust-stubs.c)
  target_include_directories(
    test_windows_keys_steps_rust
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}"
      "${CMAKE_SOURCE_DIR}/libobs"
      "${CMAKE_SOURCE_DIR}/deps/libcaption"
  )
  target_link_libraries(
    test_windows_keys_steps_rust
    PRIVATE
      OBS::libobs
      OBS::caption
      Uthash::Uthash
      ${CMOCKA_LIBRARIES}
      libobs_rust-static
      jansson::jansson
      $<TARGET_NAME_IF_EXISTS:LibUUID::LibUUID>
      $<TARGET_NAME_IF_EXISTS:gio::gio>
      $<$<TARGET_EXISTS:gio::gio>:gobject-2.0>
      $<$<TARGET_EXISTS:gio::gio>:glib-2.0>
  )
  obs_test_sanitize(test_windows_keys_steps_rust "address,undefined")
  # The staticlib's obs-util host halves call the platform directly: on Windows
  # UuidCreate(), timeBeginPeriod() and timeEndPeriod(); on macOS the
  # Objective-C runtime, CoreFoundation and IOKit. libobs links these per host
  # in libobs/cmake/os-<os>.cmake, so the executable does the same.
  if(WIN32)
    target_link_libraries(test_windows_keys_steps_rust PRIVATE winmm Rpcrt4)
  elseif(OS_MACOS)
    target_link_libraries(
      test_windows_keys_steps_rust
      PRIVATE
        "$<LINK_LIBRARY:FRAMEWORK,AppKit.framework>"
        "$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>"
        "$<LINK_LIBRARY:FRAMEWORK,Foundation.framework>"
        "$<LINK_LIBRARY:FRAMEWORK,IOKit.framework>"
    )
  endif()
  add_dependencies(cmocka-tests test_windows_keys_steps_rust)
  add_test(test_windows_keys_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_windows_keys_steps_rust)
endif()
