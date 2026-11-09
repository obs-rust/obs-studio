# A Rust archive member can pull this setter into unrelated source-step tests.
# Link the real adapter there as well as in production libobs.
function(_encoder_set_preferred_format_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      get_target_property(_sources ${_target} SOURCES)
      set(_adapter "${CMAKE_CURRENT_SOURCE_DIR}/encoder-set-preferred-format-link-support.c")
      if(NOT _adapter IN_LIST _sources)
        target_sources(${_target} PRIVATE "${_adapter}")
        target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
        target_link_libraries(
          ${_target}
          PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
        )
      endif()
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _encoder_set_preferred_format_link_support)
