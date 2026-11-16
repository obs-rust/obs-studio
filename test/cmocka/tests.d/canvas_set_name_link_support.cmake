# Keep the real adapter available to standalone Rust-static consumers. An
# archive prevents unrelated step tests from pulling the canvas/core globals.
function(_canvas_set_name_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET canvas_set_name_link_support)
        set(_adapter "${CMAKE_SOURCE_DIR}/libobs/obs-canvas-set-name-fields.c.h")
        set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_adapter}")
        file(READ "${_adapter}" _source)
        string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "#include \"obs-internal.h\"" _source "${_source}")
        set(_generated "${CMAKE_CURRENT_BINARY_DIR}/canvas-set-name-link-support.c")
        file(WRITE "${_generated}" "${_source}")
        add_library(canvas_set_name_link_support STATIC EXCLUDE_FROM_ALL "${_generated}")
        target_include_directories(
          canvas_set_name_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          canvas_set_name_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE canvas_set_name_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _canvas_set_name_link_support)
