# The Windows-key archive fixture supplies the existing hotkey seams. Link
# the production app-ID adapter too, including in legacy non-graph targets.
function(_hotkey_background_app_id_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    get_target_property(_sources ${_target} SOURCES)
    if("libobs_rust-static" IN_LIST _links AND _sources MATCHES "windows-keys-rust-stubs[.]c")
      set(
        _adapter
        "${CMAKE_CURRENT_BINARY_DIR}/native-tests/${_target}/libobs/obs-hotkey-background-app-id-fields.c.h.c"
      )
      if(NOT _adapter IN_LIST _sources)
        file(
          GENERATE OUTPUT "${_adapter}"
          CONTENT "#include \"${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-background-app-id-fields.c.h\"\n"
        )
        target_sources(${_target} PRIVATE "${_adapter}")
      endif()
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _hotkey_background_app_id_link_support)
