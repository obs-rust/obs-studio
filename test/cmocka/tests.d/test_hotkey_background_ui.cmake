# Unchanged headless helper is registered in build/test-graph.d (Refs #1390).
set(HAVE_HOTKEY_BACKGROUND_JSON FALSE)
if(OS_LINUX OR OS_FREEBSD)
  find_package(nlohmann_json 3.11 QUIET)
  if(TARGET nlohmann_json::nlohmann_json)
    set(HAVE_HOTKEY_BACKGROUND_JSON TRUE)
  endif()
endif()
