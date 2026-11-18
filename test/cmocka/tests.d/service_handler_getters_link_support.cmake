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
  if(TARGET test_service_handler_getters_rust)
    # The real Rust logger selects obs-util host members, just as in the
    # standalone video-render fixture. GC does not replace their link support.
    if(TARGET gio::gio)
      find_package(PkgConfig REQUIRED)
      pkg_check_modules(_service_handler_getters_gio REQUIRED IMPORTED_TARGET gio-2.0 glib-2.0 gobject-2.0)
      target_link_libraries(test_service_handler_getters_rust PRIVATE PkgConfig::_service_handler_getters_gio)
    endif()
    if(TARGET LibUUID::LibUUID)
      target_link_libraries(test_service_handler_getters_rust PRIVATE LibUUID::LibUUID)
    endif()
    if(WIN32)
      target_link_libraries(test_service_handler_getters_rust PRIVATE Rpcrt4 winmm)
    endif()
    # MSVC retains the filename formatter's video-info reference. Keep the
    # existing no-call provider separate from the real logger and field adapter.
    add_library(
      service_handler_getters_platform_link_support
      STATIC
      "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-video-link-stub.c"
    )
    target_link_libraries(
      test_service_handler_getters_rust
      PRIVATE libobs_rust-static service_handler_getters_platform_link_support
    )
  endif()
endfunction()
cmake_language(DEFER CALL _service_handler_getters_link_support)
