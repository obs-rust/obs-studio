# Link the real private adapter only when a Rust-static consumer needs it.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-enum-scenes-fields.c.h" _enum_scenes_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _enum_scenes_adapter "${_enum_scenes_adapter}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/enum-scenes-fields.c" "#include \"obs-internal.h\"\n${_enum_scenes_adapter}")
function(_enum_scenes_link_support)
  add_library(enum_scenes_link_support STATIC "${CMAKE_CURRENT_BINARY_DIR}/enum-scenes-fields.c")
  target_include_directories(enum_scenes_link_support PRIVATE "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config")
  target_link_libraries(
    enum_scenes_link_support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE enum_scenes_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _enum_scenes_link_support)
