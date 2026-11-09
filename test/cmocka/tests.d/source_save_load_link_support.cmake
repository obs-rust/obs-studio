# Archive the real adapter so unrelated source-step consumers do not pull its
# signal helper (which correctly depends on the live OBS core).
function(_source_save_load_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  set(_support obs_source_save_load_link_support)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      if(NOT TARGET ${_support})
        add_library(${_support} STATIC "${CMAKE_CURRENT_SOURCE_DIR}/source-save-load-link-support.c")
        target_include_directories(
          ${_support}
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          ${_support}
          PRIVATE
            OBS::caption
            Uthash::Uthash
            SIMDe::SIMDe
            Threads::Threads
            "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE ${_support})
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_save_load_link_support)
