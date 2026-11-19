# Keep the real private adapter available to standalone Rust-static tests.
function(_graphics_samplerstate_lifecycle_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_" AND "libobs_rust-static" IN_LIST _links)
      if(NOT TARGET obs-graphics-samplerstate-lifecycle-link-support)
        add_library(
          obs-graphics-samplerstate-lifecycle-link-support
          STATIC
          graphics-samplerstate-lifecycle-link-support.c
        )
        target_include_directories(
          obs-graphics-samplerstate-lifecycle-link-support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
        )
        target_link_libraries(
          obs-graphics-samplerstate-lifecycle-link-support
          PRIVATE SIMDe::SIMDe Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE obs-graphics-samplerstate-lifecycle-link-support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _graphics_samplerstate_lifecycle_link_support)
