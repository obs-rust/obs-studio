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

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). obs_windows_keys_* are
# hidden in the shared library, so the test takes them from the production
# staticlib at executable link time and compiles none of the C. The
# staticlib's obs-hotkey and obs-util members also reference libobs's hidden
# hotkey seams and logging halves, which libobs links but does not export;
# windows-keys-rust-stubs.c stands in for them (each aborts: the shims call
# none of them). Those members also need obs-util's system libraries, which the
# executable links as libobs does. Linux only: the shims have no host code, so
# one host shows the swap, and the macOS and Windows links of obs-util's
# members are not worked out here.
if(TARGET libobs_rust-static)
  if(OS_LINUX)
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
        LibUUID::LibUUID
        $<TARGET_NAME_IF_EXISTS:gio::gio>
        $<$<TARGET_EXISTS:gio::gio>:gobject-2.0>
        $<$<TARGET_EXISTS:gio::gio>:glib-2.0>
    )
    obs_test_sanitize(test_windows_keys_steps_rust "address,undefined")
    add_dependencies(cmocka-tests test_windows_keys_steps_rust)
    add_test(test_windows_keys_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_windows_keys_steps_rust)
  endif()
endif()
