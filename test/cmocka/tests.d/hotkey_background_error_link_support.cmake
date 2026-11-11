# Link the real adapter only into the portal fixture, which owns hotkey_test_obs.
# Unrelated fixtures must not pull this global-reading release archive member.
function(_hotkey_background_error_link_support)
  if(TARGET test_portal_shortcuts)
    get_target_property(_sources test_portal_shortcuts SOURCES)
    set(_adapter "${CMAKE_CURRENT_SOURCE_DIR}/hotkey-background-error-link-support.c")
    if(NOT _adapter IN_LIST _sources)
      target_sources(test_portal_shortcuts PRIVATE "${_adapter}")
    endif()
  endif()
endfunction()
cmake_language(DEFER CALL _hotkey_background_error_link_support)
