# Keep the real private adapter available to legacy Rust-static tests.
# An archive leaves it unselected when the test uses only pure graphics helpers.
function(_raster_entrypoints_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_" AND "libobs_rust-static" IN_LIST _links)
      if(NOT TARGET obs-raster-entrypoints-link-support)
        add_library(obs-raster-entrypoints-link-support STATIC raster-entrypoints-link-support.c)
        target_include_directories(
          obs-raster-entrypoints-link-support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
        )
        target_link_libraries(
          obs-raster-entrypoints-link-support
          PRIVATE SIMDe::SIMDe Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE obs-raster-entrypoints-link-support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _raster_entrypoints_link_support)
