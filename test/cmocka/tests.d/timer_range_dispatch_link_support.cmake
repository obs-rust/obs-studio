# Use the real adapter for standalone Rust-static consumers.
function(_timer_range_dispatch_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET timer_range_dispatch_link_support)
        file(
          WRITE "${CMAKE_CURRENT_BINARY_DIR}/timer-range-dispatch-link-support.c"
          "#include \"libobs/obs-timer-range-dispatch-fields.c.h\"\n"
        )
        add_library(
          timer_range_dispatch_link_support
          STATIC
          "${CMAKE_CURRENT_BINARY_DIR}/timer-range-dispatch-link-support.c"
        )
        target_include_directories(
          timer_range_dispatch_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          timer_range_dispatch_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE timer_range_dispatch_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _timer_range_dispatch_link_support)
