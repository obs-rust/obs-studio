# Link the real production adapter, with its broad PCH include replaced by
# the private layouts it actually uses, into standalone Rust-static consumers.
set(_lookup_fields "${CMAKE_SOURCE_DIR}/libobs/obs-scene-lookup-fields.c.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_lookup_fields}")
file(READ "${_lookup_fields}" _lookup_body)
string(
  REPLACE
  "#include \"libobs/build/libobs.pch.h\""
  "#include \"obs-internal.h\"\n#include \"obs-scene.h\""
  _lookup_body
  "${_lookup_body}"
)
set(_lookup_support "${CMAKE_CURRENT_BINARY_DIR}/scene-lookup-link-support.c")
file(CONFIGURE OUTPUT "${_lookup_support}" CONTENT "${_lookup_body}" @ONLY)
function(_scene_lookup_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_scene" AND "libobs_rust-static" IN_LIST _links)
      target_sources(${_target} PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/scene-lookup-link-support.c")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(
        ${_target}
        PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
      )
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _scene_lookup_link_support)
