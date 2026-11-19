# Link the real field/lock adapters for the unchanged injection recorder.
function(_hotkey_inject_entry_link_support)
  if(TARGET libobs_rust-static AND TARGET test_hotkey_inject_entry)
    get_target_property(_links test_hotkey_inject_entry LINK_LIBRARIES)
    get_target_property(_sources test_hotkey_inject_entry SOURCES)
    get_target_property(_graph_sources test_hotkey_inject_entry OBS_NATIVE_TEST_GRAPH_SOURCES)
    set(_adapter "${CMAKE_CURRENT_SOURCE_DIR}/hotkey-inject-entry-link-support.c")
    set(_graph_adapter "test/cmocka/hotkey-inject-entry-link-support.c")
    if(
      "libobs_rust-static" IN_LIST _links
      AND NOT _adapter IN_LIST _sources
      AND NOT _graph_adapter IN_LIST _graph_sources
    )
      target_sources(test_hotkey_inject_entry PRIVATE "${_adapter}")
    endif()
  endif()
endfunction()
cmake_language(DEFER CALL _hotkey_inject_entry_link_support)
