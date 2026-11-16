# Compile the real adapter for standalone Rust-static source-step consumers.
# Replace only its broad production PCH with the private layout header.
function(_source_enabled_setter_link_support)
  if(TARGET gio::gio)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(_source_enabled_setter_gio REQUIRED IMPORTED_TARGET gio-2.0 glib-2.0 gobject-2.0)
  endif()
  set(_source "${CMAKE_SOURCE_DIR}/libobs/obs-source-enabled-setter-fields.c.h")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source}")
  file(READ "${_source}" _adapter_text)
  string(
    REPLACE
    "#include \"libobs/build/libobs.pch.h\""
    "#include \"obs-internal.h\""
    _adapter_text
    "${_adapter_text}"
  )
  set(_adapter "${CMAKE_CURRENT_BINARY_DIR}/source-enabled-setter-link-support.c")
  file(WRITE "${_adapter}" "${_adapter_text}")
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      # Calldata allocation pulls obs-util host/log members on every platform.
      # Separate archive members let fixture-owned service definitions win.
      if(NOT TARGET source_enabled_setter_platform_link_support)
        add_library(
          source_enabled_setter_platform_link_support
          STATIC
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-crash-link-stub.c"
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-log-link-stubs.c"
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-video-link-stub.c"
        )
        target_include_directories(source_enabled_setter_platform_link_support PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      endif()
      target_link_libraries(${_target} PRIVATE source_enabled_setter_platform_link_support)
      if(WIN32)
        target_link_libraries(${_target} PRIVATE Rpcrt4 winmm)
      endif()
      foreach(_host_library IN ITEMS LibUUID::LibUUID PkgConfig::_source_enabled_setter_gio)
        if(TARGET ${_host_library})
          target_link_libraries(${_target} PRIVATE ${_host_library})
        endif()
      endforeach()
      target_sources(${_target} PRIVATE "${_adapter}")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(
        ${_target}
        PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
      )
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_enabled_setter_link_support)
