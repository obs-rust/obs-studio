# Real constructor adapters, lazily linked for standalone Rust-static consumers.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-scene-create-fields.c.h" _scene_create_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _scene_create_adapter "${_scene_create_adapter}")
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/scene-create-fields.c"
  "#include \"obs-internal.h\"\n#include \"obs-scene.h\"\n${_scene_create_adapter}"
)
function(_scene_create_link_support)
  add_library(scene_create_link_support STATIC "${CMAKE_CURRENT_BINARY_DIR}/scene-create-fields.c")
  target_include_directories(
    scene_create_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    scene_create_link_support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE scene_create_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _scene_create_link_support)
