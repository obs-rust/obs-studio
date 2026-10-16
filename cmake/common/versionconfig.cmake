# OBS CMake common version helper module

include_guard(GLOBAL)

set(_obs_version ${_obs_default_version})
set(_obs_version_canonical ${_obs_default_version})

# Attempt to automatically discover expected OBS version
if(NOT DEFINED OBS_VERSION_OVERRIDE AND EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/.git")
  execute_process(
    COMMAND git describe --always --tags --match "[0-9]*.[0-9]*.[0-9]*" --dirty=-modified
    OUTPUT_VARIABLE _obs_version
    ERROR_VARIABLE _git_describe_err
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    RESULT_VARIABLE _obs_version_result
    OUTPUT_STRIP_TRAILING_WHITESPACE
  )

  if(_git_describe_err)
    message(FATAL_ERROR "Could not fetch OBS version tag from git.\n" ${_git_describe_err})
  endif()

  if(_obs_version_result EQUAL 0 AND _obs_version MATCHES "^[0-9]+\\.[0-9]+\\.[0-9]+")
    string(REGEX REPLACE "([0-9]+)\\.([0-9]+)\\.([0-9]+).*" "\\1;\\2;\\3" _obs_version_canonical ${_obs_version})
  else()
    # Git forks without OBS release tags make git describe return a commit SHA.
    # Keep the bootstrap default, just as in a source archive without .git.
    set(_obs_version ${_obs_default_version})
  endif()
elseif(DEFINED OBS_VERSION_OVERRIDE)
  if(OBS_VERSION_OVERRIDE MATCHES "([0-9]+)\\.([0-9]+)\\.([0-9]+).*")
    string(
      REGEX REPLACE
      "([0-9]+)\\.([0-9]+)\\.([0-9]+).*"
      "\\1;\\2;\\3"
      _obs_version_canonical
      ${OBS_VERSION_OVERRIDE}
    )
    set(_obs_version ${OBS_VERSION_OVERRIDE})

    # The override is the OBS version libobs reports to plugins
    # (obs_get_version_string()), which they compare against the OBS version
    # they need. An override older than the libobs API this tree builds is
    # almost always the fork's own version passed in the wrong variable (#1087).
    file(
      STRINGS "${CMAKE_CURRENT_LIST_DIR}/../../libobs/obs-config.h"
      _obs_api_major
      REGEX "^#define LIBOBS_API_MAJOR_VER [0-9]+$"
    )
    string(REGEX REPLACE "^#define LIBOBS_API_MAJOR_VER ([0-9]+)$" "\\1" _obs_api_major "${_obs_api_major}")
    if(NOT _obs_api_major MATCHES "^[0-9]+$")
      message(FATAL_ERROR "Could not read LIBOBS_API_MAJOR_VER from libobs/obs-config.h.")
    endif()
    list(GET _obs_version_canonical 0 _obs_override_major)
    if(_obs_override_major LESS _obs_api_major)
      message(
        FATAL_ERROR
        "OBS_VERSION_OVERRIDE '${OBS_VERSION_OVERRIDE}' is older than the libobs API this tree builds "
        "(LIBOBS_API_MAJOR_VER ${_obs_api_major} in libobs/obs-config.h).\n"
        "OBS_VERSION_OVERRIDE sets the OBS version that libobs reports to plugins "
        "(obs_get_version_string()); plugins refuse to load when it is older than the OBS they need.\n"
        "Pass the OBS-Studio-Rust version (e.g. 0.1.0-beta.20261014) as -DOBS_RUST_VERSION=... instead "
        "and leave OBS_VERSION_OVERRIDE unset: CMake then takes the OBS version from the upstream tags "
        "(git describe --match '[0-9]*.[0-9]*.[0-9]*')."
      )
    endif()
    unset(_obs_api_major)
    unset(_obs_override_major)
  else()
    message(FATAL_ERROR "Invalid version supplied - must be <MAJOR>.<MINOR>.<PATCH>[-(rc|beta)<NUMBER>].")
  endif()
endif()

# Set beta/rc versions if suffix included in version string
if(_obs_version MATCHES "[0-9]+\\.[0-9]+\\.[0-9]+-rc[0-9]+")
  string(REGEX REPLACE "[0-9]+\\.[0-9]+\\.[0-9]+-rc([0-9]+).*$" "\\1" _obs_release_candidate ${_obs_version})
elseif(_obs_version MATCHES "[0-9]+\\.[0-9]+\\.[0-9]+-beta[0-9]+")
  string(REGEX REPLACE "[0-9]+\\.[0-9]+\\.[0-9]+-beta([0-9]+).*$" "\\1" _obs_beta ${_obs_version})
endif()

list(GET _obs_version_canonical 0 OBS_VERSION_MAJOR)
list(GET _obs_version_canonical 1 OBS_VERSION_MINOR)
list(GET _obs_version_canonical 2 OBS_VERSION_PATCH)

set(OBS_RELEASE_CANDIDATE ${_obs_release_candidate})
set(OBS_BETA ${_obs_beta})

string(REPLACE ";" "." OBS_VERSION_CANONICAL "${_obs_version_canonical}")
string(REPLACE ";" "." OBS_VERSION "${_obs_version}")

if(OBS_RELEASE_CANDIDATE GREATER 0)
  message(
    AUTHOR_WARNING
    "******************************************************************************\n"
    "  + OBS-Studio - Release candidate detected, OBS_VERSION is now: ${OBS_VERSION}\n"
    "******************************************************************************"
  )
elseif(OBS_BETA GREATER 0)
  message(
    AUTHOR_WARNING
    "******************************************************************************\n"
    "  + OBS-Studio - Beta detected, OBS_VERSION is now: ${OBS_VERSION}\n"
    "******************************************************************************"
  )
endif()

# OBS-Studio-Rust release string (e.g. 0.1.0-beta.20261013), set by the release
# workflow through the OBS_RUST_VERSION environment variable or cache entry.
# Empty for local builds, which use the libobs-rust crate version (#631).
set(OBS_RUST_RELEASE_VERSION "")
if(DEFINED OBS_RUST_VERSION AND NOT "${OBS_RUST_VERSION}" STREQUAL "")
  set(OBS_RUST_RELEASE_VERSION "${OBS_RUST_VERSION}")
elseif(NOT "$ENV{OBS_RUST_VERSION}" STREQUAL "")
  set(OBS_RUST_RELEASE_VERSION "$ENV{OBS_RUST_VERSION}")
endif()
if(OBS_RUST_RELEASE_VERSION AND NOT OBS_RUST_RELEASE_VERSION MATCHES "^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.]+)?$")
  message(
    FATAL_ERROR
    "Invalid OBS_RUST_VERSION '${OBS_RUST_RELEASE_VERSION}' - must be <MAJOR>.<MINOR>.<PATCH>[-<suffix>]."
  )
endif()

unset(_obs_default_version)
unset(_obs_version)
unset(_obs_version_canonical)
unset(_obs_release_candidate)
unset(_obs_beta)
unset(_obs_version_result)
