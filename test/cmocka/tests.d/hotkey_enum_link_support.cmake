# Standalone Rust-static consumers own hotkey_test_obs and use the real adapter.
function(_hotkey_enum_link_support)
  if(TARGET libobs_rust-static)
    foreach(
      _target
      test_hotkey_background_enum
      test_hotkey_binding_press_steps
      test_hotkey_getters_characterization
      test_hotkey_load
      test_hotkey_pair_steps
      test_hotkey_press_steps
      test_portal_shortcuts
      test_wayland_hotkeys
    )
      if(TARGET ${_target})
        get_target_property(_links ${_target} LINK_LIBRARIES)
        get_target_property(_sources ${_target} SOURCES)
        get_target_property(_graph_sources ${_target} OBS_NATIVE_TEST_GRAPH_SOURCES)
        set(_adapter "${CMAKE_CURRENT_SOURCE_DIR}/hotkey-enum-link-support.c")
        set(_graph_adapter "test/cmocka/hotkey-enum-link-support.c")
        if(
          "libobs_rust-static" IN_LIST _links
          AND NOT _adapter IN_LIST _sources
          AND NOT _graph_adapter IN_LIST _graph_sources
        )
          target_sources(${_target} PRIVATE "${_adapter}")
        endif()
      endif()
    endforeach()
  endif()
endfunction()
cmake_language(DEFER CALL _hotkey_enum_link_support)
