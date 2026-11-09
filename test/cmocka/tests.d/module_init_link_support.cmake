# Live adapters are an archive: unrelated Rust-static fixtures need not pull
# module lifecycle services (the real OBS core owns the profiler name store).
function(_module_init_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET module_init_link_support)
        add_library(module_init_link_support STATIC "${CMAKE_CURRENT_SOURCE_DIR}/module-init-link-support.c")
        target_include_directories(module_init_link_support PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
        target_link_libraries(
          module_init_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE module_init_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _module_init_link_support)
