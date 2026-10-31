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
# - LIBOBS_PORTED_SOURCES: C with a Rust port in rust/libobs-rust. Only the
#   explicit C reference/rollback compiles these; production links Rust instead.
# - LIBOBS_RUST_SOURCES: the C halves the hybrid adds (C variadics).
# - LIBOBS_<HOST>_SOURCES / _PORTED_SOURCES: the same split, per host.
# - LIBOBS_HEVC_PORTED_SOURCES, LIBOBS_PULSE_PORTED_SOURCES,
#   LIBOBS_NIX_GIO_PORTED_SOURCES,
#   LIBOBS_WAYLAND_PORTED_SOURCES, LIBOBS_WAYLAND_GIO_PORTED_SOURCES: ported
#   C behind ENABLE_HEVC (below), ENABLE_PULSEAUDIO, GIO, ENABLE_WAYLAND, and
#   ENABLE_WAYLAND with GIO (os-linux.cmake, os-freebsd.cmake).
# - No-op host capabilities (#1051) are not listed: each host file includes
#   the obs-host/noop/<capability>.hpp headers for what it lacks.
# <HOST> is WINDOWS, MACOS or LINUX; LINUX also covers FreeBSD and OpenBSD,
# which build obs-nix.c the same way.

# Canonical source lists are shared with obs-build and bundled obs-sys: one
# JSON list per set in build/source-sets.d/<SET>.json (#3235), so a port edits
# only the lists it touches and a host set's file name carries its OS for CI
# scoping (ci/platform_paths.py). CONFIGURE_DEPENDS re-runs configure when a
# set file is added or removed; each file read is a configure dependency too.
file(GLOB _libobs_source_set_files CONFIGURE_DEPENDS "${CMAKE_CURRENT_LIST_DIR}/../build/source-sets.d/*.json")
if(NOT _libobs_source_set_files)
  message(FATAL_ERROR "No libobs source sets in ${CMAKE_CURRENT_LIST_DIR}/../build/source-sets.d")
endif()
# The set names, for the port fragment check below.
set(_libobs_source_set_names "")
foreach(_set_file IN LISTS _libobs_source_set_files)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_set_file}")
  get_filename_component(_name "${_set_file}" NAME_WE)
  list(APPEND _libobs_source_set_names "${_name}")
  file(READ "${_set_file}" _set_members)
  string(JSON _source_count LENGTH "${_set_members}")
  set(${_name} "")
  set(${_name}_HEADERS "")
  if(_source_count GREATER 0)
    math(EXPR _source_last "${_source_count} - 1")
    foreach(_source_index RANGE ${_source_last})
      string(JSON _source GET "${_set_members}" ${_source_index})
      list(APPEND ${_name} "${_source}")
    endforeach()
  endif()
endforeach()

# Each port registers itself in build/ported-source-sets/<unit>.json (#3236), so
# it edits no shared file (not this one, libobs/CMakeLists.txt or the
# cmake/os-<os>.cmake files):
#
#   {
#     "LIBOBS_<...>PORTED_SOURCES": ["build/libobs.ported-<unit>+.h.c"],
#     "compile_options": {"gnu": ["-ffp-contract=off"], "msvc": ["/wd4056"]},
#     "headers": ["<unit>.h"]
#   }
#
# Set keys (one or more) add the unit's groups to those sets. Optional
# compile_options are appended to each of the unit's groups, per compiler family:
# gnu is GCC, Clang and AppleClang (C and Objective-C), msvc is MSVC. They are
# source properties of the including directory (libobs/), as the per-group
# set_property blocks in libobs/CMakeLists.txt were. Optional
# headers (relative to libobs/) are listed on libobs-objects wherever one of the
# unit's sets is built (libobs_add_ported_sources below), for IDEs and install
# checks; they compile nothing.
file(GLOB _libobs_port_fragments CONFIGURE_DEPENDS "${CMAKE_CURRENT_LIST_DIR}/../build/ported-source-sets/*.json")
foreach(_fragment IN LISTS _libobs_port_fragments)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_fragment}")
  file(READ "${_fragment}" _fragment_sets)
  string(JSON _fragment_count LENGTH "${_fragment_sets}")
  if(_fragment_count EQUAL 0)
    message(FATAL_ERROR "Empty libobs port fragment: ${_fragment}")
  endif()
  math(EXPR _fragment_last "${_fragment_count} - 1")
  set(_unit_sets "")
  set(_unit_groups "")
  foreach(_fragment_index RANGE ${_fragment_last})
    string(JSON _name MEMBER "${_fragment_sets}" ${_fragment_index})
    if(_name STREQUAL "compile_options" OR _name STREQUAL "headers")
      continue()
    endif()
    if(NOT _name MATCHES "^LIBOBS_(.*_)?PORTED_SOURCES$")
      message(FATAL_ERROR "Unknown libobs ported source set ${_name} in ${_fragment}")
    endif()
    if(NOT _name IN_LIST _libobs_source_set_names)
      message(FATAL_ERROR "Unknown libobs ported source set ${_name} in ${_fragment}")
    endif()
    list(APPEND _unit_sets "${_name}")
    string(JSON _source_count LENGTH "${_fragment_sets}" "${_name}")
    if(_source_count GREATER 0)
      math(EXPR _source_last "${_source_count} - 1")
      foreach(_source_index RANGE ${_source_last})
        string(JSON _source GET "${_fragment_sets}" "${_name}" ${_source_index})
        if(_source IN_LIST ${_name})
          message(FATAL_ERROR "Duplicate libobs ported source ${_source} in ${_fragment}")
        endif()
        list(APPEND ${_name} "${_source}")
        list(APPEND _unit_groups "${_source}")
      endforeach()
    endif()
    list(SORT ${_name})
  endforeach()
  if(NOT _unit_sets)
    message(FATAL_ERROR "libobs port fragment ${_fragment} names no ported source set")
  endif()

  string(JSON _options ERROR_VARIABLE _missing GET "${_fragment_sets}" "compile_options")
  if(NOT _missing)
    string(JSON _family_count LENGTH "${_options}")
    math(EXPR _family_last "${_family_count} - 1")
    foreach(_family_index RANGE ${_family_last})
      string(JSON _family MEMBER "${_options}" ${_family_index})
      if(_family STREQUAL "gnu")
        set(_condition "C_COMPILER_ID:GNU,Clang,AppleClang")
      elseif(_family STREQUAL "msvc")
        set(_condition "C_COMPILER_ID:MSVC")
      else()
        message(FATAL_ERROR "Unknown compiler family ${_family} in ${_fragment} (gnu or msvc)")
      endif()
      string(JSON _flag_count LENGTH "${_options}" "${_family}")
      math(EXPR _flag_last "${_flag_count} - 1")
      set(_flags "")
      foreach(_flag_index RANGE ${_flag_last})
        string(JSON _flag GET "${_options}" "${_family}" ${_flag_index})
        list(APPEND _flags "${_flag}")
      endforeach()
      set_property(SOURCE ${_unit_groups} APPEND PROPERTY COMPILE_OPTIONS "$<$<${_condition}>:${_flags}>")
    endforeach()
  endif()

  string(JSON _headers ERROR_VARIABLE _missing GET "${_fragment_sets}" "headers")
  if(NOT _missing)
    string(JSON _header_count LENGTH "${_headers}")
    math(EXPR _header_last "${_header_count} - 1")
    foreach(_header_index RANGE ${_header_last})
      string(JSON _header GET "${_headers}" ${_header_index})
      foreach(_name IN LISTS _unit_sets)
        list(APPEND ${_name}_HEADERS "${_header}")
      endforeach()
    endforeach()
  endif()
endforeach()

if(ENABLE_HEVC)
  list(APPEND LIBOBS_SOURCES obs-hevc.h)
  list(APPEND LIBOBS_PORTED_SOURCES ${LIBOBS_HEVC_PORTED_SOURCES})
  list(APPEND LIBOBS_PORTED_SOURCES_HEADERS ${LIBOBS_HEVC_PORTED_SOURCES_HEADERS})
endif()

# Builds one ported set: its groups into the explicit C targets only (production
# links the Rust port instead), its units' headers onto libobs-objects.
function(libobs_add_ported_sources set)
  obs_libobs_add_c_sources(${${set}})
  if(${set}_HEADERS)
    target_sources(libobs-objects PRIVATE ${${set}_HEADERS})
  endif()
endfunction()

# Adds a host's lists (libobs/CMakeLists.txt): its sources to the
# objects both libobs binaries share, its ported C to the explicit C targets.
macro(libobs_add_host_sources host)
  target_sources(libobs-objects PRIVATE ${LIBOBS_${host}_SOURCES})
  libobs_add_ported_sources(LIBOBS_${host}_PORTED_SOURCES)
endmacro()
