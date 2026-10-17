# libobs source lists, relative to libobs/. The real build (libobs/CMakeLists.txt
# and cmake/os-<os>.cmake) and the cross-compile check (cmake/cross/) both read
# them, so a cross check compiles exactly the files that host's build would.
#
# Sources are unity groups (BUILD.md): build/<name>+.h.c (.h.m for
# Objective-C) #includes the implementation files (foo.c.h) it compiles. A
# file moves between lists by moving its #include line between groups.
#
# Per list:
# - LIBOBS_SOURCES: built on every host, no Rust port.
# - LIBOBS_PORTED_SOURCES: C with a Rust port in rust/libobs-rust. libobs
#   compiles them; libobs-with-rust links the staticlib instead (#1065).
# - LIBOBS_RUST_SOURCES: the C halves only libobs-with-rust adds (C variadics).
# - LIBOBS_<HOST>_SOURCES / _PORTED_SOURCES: the same split, per host.
# - No-op host capabilities (#1051) are not listed: each host file includes
#   the obs-host/noop/<capability>.hpp headers for what it lacks.
# <HOST> is WINDOWS, MACOS or LINUX; LINUX also covers FreeBSD and OpenBSD,
# which build obs-nix.c the same way.

# Canonical source lists are shared with obs-build and bundled obs-sys.
set(_libobs_source_manifest "${CMAKE_CURRENT_LIST_DIR}/../build/source-sets.json")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_libobs_source_manifest}")
file(READ "${_libobs_source_manifest}" _libobs_source_sets)
string(JSON _set_count LENGTH "${_libobs_source_sets}")
math(EXPR _set_last "${_set_count} - 1")
foreach(_set_index RANGE ${_set_last})
  string(JSON _name MEMBER "${_libobs_source_sets}" ${_set_index})
  string(JSON _source_count LENGTH "${_libobs_source_sets}" "${_name}")
  set(${_name} "")
  if(_source_count GREATER 0)
    math(EXPR _source_last "${_source_count} - 1")
    foreach(_source_index RANGE ${_source_last})
      string(JSON _source GET "${_libobs_source_sets}" "${_name}" ${_source_index})
      list(APPEND ${_name} "${_source}")
    endforeach()
  endif()
endforeach()

if(ENABLE_HEVC)
  list(APPEND LIBOBS_SOURCES obs-hevc.h)
  list(APPEND LIBOBS_PORTED_SOURCES build/libobs.ported-hevc+.h.c)
endif()

# Adds a host's lists (libobs/CMakeLists.txt): its sources to the
# objects both libobs binaries share, its ported C to libobs only
# (libobs-with-rust gets the staticlib instead).
macro(libobs_add_host_sources host)
  target_sources(libobs-objects PRIVATE ${LIBOBS_${host}_SOURCES})
  target_sources(libobs PRIVATE ${LIBOBS_${host}_PORTED_SOURCES})
endmacro()
