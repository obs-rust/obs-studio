# Every standalone Rust-static consumer can select the real field adapter.
function(_service_handler_getters_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET service_handler_getters_link_support)
        add_library(
          service_handler_getters_link_support
          STATIC
          "${CMAKE_CURRENT_SOURCE_DIR}/service-handler-getters-link-support.c"
        )
        target_include_directories(
          service_handler_getters_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          service_handler_getters_link_support
          PRIVATE OBS::caption Uthash::Uthash SIMDe::SIMDe "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE service_handler_getters_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _service_handler_getters_link_support)
