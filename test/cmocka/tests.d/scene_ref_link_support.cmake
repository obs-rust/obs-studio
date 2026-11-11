# Real source operations are pulled only by consumers of the scene ref shim.
function(_scene_ref_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET scene_ref_link_support)
        add_library(scene_ref_link_support STATIC "${CMAKE_CURRENT_SOURCE_DIR}/scene-ref-link-support.c")
        target_include_directories(
          scene_ref_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          scene_ref_link_support
          PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
        )
      endif()
      target_link_libraries(${_target} PRIVATE scene_ref_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _scene_ref_link_support)
