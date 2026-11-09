# Compile the real production adapter, stripping only its broad PCH include.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-scene-item-locked-fields.c.h" _locked_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _locked_adapter "${_locked_adapter}")
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/scene-item-locked-fields.c"
  "#include \"obs-internal.h\"\n#include \"obs-scene.h\"\n${_locked_adapter}"
)
function(_scene_item_locked_setter_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_scene" AND "libobs_rust-static" IN_LIST _links)
      target_sources(${_target} PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/scene-item-locked-fields.c")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(
        ${_target}
        PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
      )
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _scene_item_locked_setter_link_support)
