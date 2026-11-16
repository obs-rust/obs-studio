# An archive supplies the real adapter only to consumers that reference it.
function(_module_post_load_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET module_post_load_link_support)
        add_library(module_post_load_link_support STATIC "${CMAKE_CURRENT_SOURCE_DIR}/module-post-load-link-support.c")
        target_include_directories(
          module_post_load_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          module_post_load_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE module_post_load_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _module_post_load_link_support)
