# Keep the real adapter available without pulling its core global into
# unrelated standalone Rust-static consumers.
function(_core_private_data_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET core_private_data_link_support)
        add_library(
          core_private_data_link_support
          STATIC
          "${CMAKE_CURRENT_SOURCE_DIR}/core-private-data-link-support.c"
        )
        target_include_directories(
          core_private_data_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          core_private_data_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE core_private_data_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _core_private_data_link_support)
