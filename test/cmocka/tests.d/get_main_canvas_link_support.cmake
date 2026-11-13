# Link the real private adapter only when a Rust-static consumer needs it.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-get-main-canvas-fields.c.h" _get_main_canvas_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _get_main_canvas_adapter "${_get_main_canvas_adapter}")
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/get-main-canvas-fields.c"
  "#include \"obs-internal.h\"\n${_get_main_canvas_adapter}"
)
function(_get_main_canvas_link_support)
  add_library(get_main_canvas_link_support STATIC "${CMAKE_CURRENT_BINARY_DIR}/get-main-canvas-fields.c")
  target_include_directories(
    get_main_canvas_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    get_main_canvas_link_support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE get_main_canvas_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _get_main_canvas_link_support)
