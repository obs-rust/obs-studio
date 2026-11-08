# Legacy source-step targets outside the native graph also pull unrelated
# getter shims from the same Rust archive members. Keep their C tests unchanged.
function(_source_state_getter_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      get_target_property(_sources ${_target} SOURCES)
      set(_stub "${CMAKE_CURRENT_SOURCE_DIR}/source-state-getter-link-stubs.c")
      if(NOT _stub IN_LIST _sources)
        target_sources(${_target} PRIVATE "${_stub}")
        target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
        target_link_libraries(
          ${_target}
          PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
        )
      endif()
      set(_log_stub "${CMAKE_CURRENT_SOURCE_DIR}/source-getter-log-link-stub.c")
      if(NOT _log_stub IN_LIST _sources)
        target_sources(${_target} PRIVATE "${_log_stub}")
      endif()
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_state_getter_link_support)
