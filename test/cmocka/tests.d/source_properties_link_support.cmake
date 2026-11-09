# Compile the real adapter for legacy Rust-static source-step consumers.
function(_source_properties_link_support)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/libobs/obs-source-properties-fields.c.h")
  file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-source-properties-fields.c.h" _adapter_body)
  string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "#include \"obs-internal.h\"" _adapter_body "${_adapter_body}")
  set(_adapter "${CMAKE_CURRENT_BINARY_DIR}/source-properties-link-support.c")
  file(WRITE "${_adapter}" "${_adapter_body}")
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      target_sources(${_target} PRIVATE "${_adapter}")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(
        ${_target}
        PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
      )
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_properties_link_support)
