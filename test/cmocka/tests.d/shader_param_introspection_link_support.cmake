# Standalone Rust-static graphics consumers use the real private adapter.
function(_shader_param_introspection_link_support)
  set(_source "${CMAKE_SOURCE_DIR}/libobs/obs-shader-param-introspection-fields.c.h")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source}")
  file(READ "${_source}" _adapter_text)
  string(
    REPLACE
    "#include \"libobs/build/libobs.pch.h\""
    "#include \"graphics/graphics-internal.h\"\n#include \"util/base.h\""
    _adapter_text
    "${_adapter_text}"
  )
  set(_adapter "${CMAKE_CURRENT_BINARY_DIR}/shader-param-introspection-link-support.c")
  file(WRITE "${_adapter}" "${_adapter_text}")
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET shader_param_introspection_link_support)
        add_library(shader_param_introspection_link_support STATIC "${_adapter}")
        target_include_directories(shader_param_introspection_link_support PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
        target_link_libraries(
          shader_param_introspection_link_support
          PRIVATE Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE shader_param_introspection_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _shader_param_introspection_link_support)
