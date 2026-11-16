# Link the real adapter only into the portal fixture, which owns hotkey_test_obs.
# Unrelated fixtures must not pull this global-reading release archive member.
function(_hotkey_background_error_link_support)
  if(TARGET test_portal_shortcuts)
    get_target_property(_sources test_portal_shortcuts SOURCES)
    get_target_property(_graph_sources test_portal_shortcuts OBS_NATIVE_TEST_GRAPH_SOURCES)
    set(_adapter "${CMAKE_CURRENT_SOURCE_DIR}/hotkey-background-error-link-support.c")
    set(_graph_adapter "test/cmocka/hotkey-background-error-link-support.c")
    if(NOT _adapter IN_LIST _sources AND NOT _graph_adapter IN_LIST _graph_sources)
      target_sources(test_portal_shortcuts PRIVATE "${_adapter}")
    endif()
  endif()
endfunction()
cmake_language(DEFER CALL _hotkey_background_error_link_support)
