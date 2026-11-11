# Compile the real production adapter, stripping only its broad PCH include.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-scene-item-locked-fields.c.h" _locked_adapter)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _locked_adapter "${_locked_adapter}")
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/scene-item-locked-fields.c"
  "#include \"obs-internal.h\"\n#include \"obs-scene.h\"\n${_locked_adapter}"
)
function(_scene_item_locked_setter_link_support)
  if(TARGET gio::gio)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(_scene_item_locked_setter_gio REQUIRED IMPORTED_TARGET gio-2.0 glib-2.0 gobject-2.0)
  endif()
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_scene" AND "libobs_rust-static" IN_LIST _links)
      # The live adapter's calldata calls pull obs-util host/log archive members.
      # Reuse the existing source-step providers in separate archive members so
      # fixture-owned bcrash/log/video definitions continue to take precedence.
      if(NOT TARGET scene_item_locked_setter_platform_link_support)
        add_library(
          scene_item_locked_setter_platform_link_support
          STATIC
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-crash-link-stub.c"
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-log-link-stubs.c"
          "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-video-link-stub.c"
        )
        target_include_directories(scene_item_locked_setter_platform_link_support PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
      endif()
      target_link_libraries(${_target} PRIVATE scene_item_locked_setter_platform_link_support)
      if(WIN32)
        target_link_libraries(${_target} PRIVATE Rpcrt4 winmm)
      endif()
      foreach(_host_library IN ITEMS LibUUID::LibUUID PkgConfig::_scene_item_locked_setter_gio)
        if(TARGET ${_host_library})
          target_link_libraries(${_target} PRIVATE ${_host_library})
        endif()
      endforeach()
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
