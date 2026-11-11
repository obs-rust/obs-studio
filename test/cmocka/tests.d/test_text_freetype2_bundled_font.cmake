# Generated fixtures for ../build/test-graph.d; no duplicate registration.
set(HAVE_FONTCONFIG_TEST_OWNER FALSE)
if(OS_LINUX AND TARGET OBS::text-freetype2)
  set(HAVE_FONTCONFIG_TEST_OWNER TRUE)
  # No <dir> entries: the host's installed fonts stay invisible (#1076).
  file(
    WRITE "${CMAKE_CURRENT_BINARY_DIR}/ft2-isolated-fonts.conf"
    "<?xml version=\"1.0\"?>\n<fontconfig><cachedir>${CMAKE_CURRENT_BINARY_DIR}/ft2-isolated-fonts-cache</cachedir></fontconfig>\n"
  )
  # A machine with the font installed system-wide (obs-rust/obs-studio#1076):
  # the same binary under a fontconfig whose system font directory holds a
  # copy of the face. The default face must still resolve to the bundled files.
  set(ft2_host_fonts "${CMAKE_CURRENT_BINARY_DIR}/ft2-host-fonts")
  file(CREATE_LINK "${CMAKE_CURRENT_LIST_DIR}/../../../plugins/text-freetype2/data/fonts" "${ft2_host_fonts}" SYMBOLIC)
  file(
    WRITE "${CMAKE_CURRENT_BINARY_DIR}/ft2-host-fonts.conf"
    "<?xml version=\"1.0\"?>\n<fontconfig><dir>${ft2_host_fonts}</dir><cachedir>${CMAKE_CURRENT_BINARY_DIR}/ft2-host-fonts-cache</cachedir></fontconfig>\n"
  )
endif()
