# Generated fixtures for ../build/test-graph.json; no duplicate registration.
set(HAVE_FONTCONFIG_TEST_OWNER FALSE)
if(OS_LINUX AND TARGET OBS::text-freetype2)
  set(HAVE_FONTCONFIG_TEST_OWNER TRUE)
  # Only the collection fixture is visible: no system font directories.
  set(ft2_collection_fonts "${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts")
  file(COPY "${CMAKE_CURRENT_LIST_DIR}/../data/text-collection.ttc" DESTINATION "${ft2_collection_fonts}")
  file(
    WRITE "${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts.conf"
    "<?xml version=\"1.0\"?>\n<fontconfig><dir>${ft2_collection_fonts}</dir><cachedir>${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts-cache</cachedir></fontconfig>\n"
  )
endif()
