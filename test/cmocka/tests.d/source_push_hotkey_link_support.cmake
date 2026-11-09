# Compile the real adapter body, without pulling it into unrelated Rust-static
# fixtures unless their Rust archive members actually reference the dispatch.
set_property(
  DIRECTORY
  APPEND
  PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/libobs/obs-source-push-hotkey-fields.c.h"
)
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-source-push-hotkey-fields.c.h" _push_hotkey_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _push_hotkey_adapter "${_push_hotkey_adapter}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/source-push-hotkey-fields.inc" "${_push_hotkey_adapter}")
add_library(source_push_hotkey_link_support STATIC source-push-hotkey-link-support.c)
target_include_directories(
  source_push_hotkey_link_support
  PRIVATE "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config" "${CMAKE_CURRENT_BINARY_DIR}"
)
target_link_libraries(
  source_push_hotkey_link_support
  PRIVATE libobs_rust-static OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
)
function(_source_push_hotkey_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE source_push_hotkey_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_push_hotkey_link_support)
