# Unchanged headless helper is registered in build/test-graph.json (Refs #1390).
set(HAVE_TEXT_FILE_OWNER FALSE)
if(TARGET OBS::text-freetype2)
  set(HAVE_TEXT_FILE_OWNER TRUE)
  find_package(Freetype REQUIRED)
endif()
