# Characterization of the window helpers' pure decisions of
# libobs/util/windows/window-helpers.c, moved to
# libobs/util/windows/window-match.c and ported to rust/obs-util
# (window_match): the internal exes and the untitled Explorer the window
# lists hide, the generic and UWP class tests, the rating ms_find_window()
# picks a window by, and the codec of the saved "title:class:exe" string.
# Builds the moved file on its own: no Windows header, and libobs only for
# astrcmpi(), astrcmpi_n() and astrstri(), so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. The Tier 3 parity tests in rust/obs-util compare the
# shims with this C on every host.
# Target and test are declared in ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target), on Linux. The port has no
# intentional difference, so the test file is the same. The functions are
# hidden in the shared library, so the test takes them from the exact
# production staticlib at executable link time; window-match.c is not compiled
# in. The shims' archive member also holds the rest of obs-util, which needs
# libuuid and GIO, and libobs's hidden base_default_log_handler() and
# base_log_literal(): this executable links the first two and compiles its own
# copy of util/base-variadic.c for the last, as test_effect_param_types_steps_rust
# does with its adapters. Linux only: obs-util's member pulls in other host code
# on macOS and Windows, and no PR lane builds this test there to check that
# link.
if(TARGET libobs_rust-static)
  if(OS_LINUX)
    find_package(LibUUID REQUIRED)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(_window_match_test_gio QUIET IMPORTED_TARGET gio-2.0 glib-2.0)
    add_executable(
      test_window_match_steps_rust
      test_window_match_steps.c
      "${CMAKE_SOURCE_DIR}/libobs/util/base-variadic.c.h"
    )
    target_include_directories(
      test_window_match_steps_rust
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
    )
    target_link_libraries(
      test_window_match_steps_rust
      PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} libobs_rust-static LibUUID::LibUUID
    )
    if(TARGET PkgConfig::_window_match_test_gio)
      target_link_libraries(test_window_match_steps_rust PRIVATE PkgConfig::_window_match_test_gio)
    endif()
    obs_test_sanitize(test_window_match_steps_rust "address,undefined")
    add_dependencies(cmocka-tests test_window_match_steps_rust)
    add_test(test_window_match_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_window_match_steps_rust)
  endif()
endif()
