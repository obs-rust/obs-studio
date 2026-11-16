# Standalone Rust-static step consumers resolve the same private adapter as libobs.
# Keep it in an archive: consumers that never call stencil APIs need no context.
function(_graphics_stencil_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "_steps_rust$" AND "libobs_rust-static" IN_LIST _links)
      if(NOT TARGET graphics-stencil-link-support)
        set(_wrapper "${CMAKE_CURRENT_BINARY_DIR}/graphics-stencil-link-support.c")
        file(WRITE "${_wrapper}" "#include \"libobs/graphics/graphics-stencil-adapter.c.h\"\n")
        add_library(graphics-stencil-link-support STATIC "${_wrapper}")
        target_include_directories(
          graphics-stencil-link-support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          graphics-stencil-link-support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE graphics-stencil-link-support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _graphics_stencil_link_support)
