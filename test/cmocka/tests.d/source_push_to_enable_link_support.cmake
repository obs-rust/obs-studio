# Link the real field adapter and signal builder into legacy Rust-static consumers.
# Extract verbatim from production, so no placeholder symbols or globals are needed.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-source-push-to-enable-fields.c.h" _push_enable_fields)
string(
  REPLACE
  "#include \"libobs/build/libobs.pch.h\""
  "#include \"obs-internal.h\""
  _push_enable_fields
  "${_push_enable_fields}"
)
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-source.c.h" _push_enable_source)
set(_push_enable_support "${CMAKE_CURRENT_BINARY_DIR}/source-push-to-enable-link-support.c")
file(WRITE "${_push_enable_support}" "#include \"obs-source-push-to-enable.h\"\n${_push_enable_fields}\n")
foreach(_function source_signal_push_to_changed obs_source_push_to_enable_signal_changed)
  string(REGEX MATCH "(static )?void ${_function}\\([^}]+\n}" _body "${_push_enable_source}")
  if(NOT _body)
    message(FATAL_ERROR "Missing production push-to signal function: ${_function}")
  endif()
  file(APPEND "${_push_enable_support}" "${_body}\n")
endforeach()
function(_source_push_to_enable_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      target_sources(${_target} PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/source-push-to-enable-link-support.c")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(
        ${_target}
        PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
      )
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_push_to_enable_link_support)
