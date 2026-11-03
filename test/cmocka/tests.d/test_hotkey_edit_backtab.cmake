# Settings > Hotkeys: Shift+Tab out of a key field bound "Shift" to that
# action before the focus moved back (obsproject/obs-studio#12475). Builds the
# frontend helper the key field calls and drives a key field made like
# OBSHotkeyEdit on Qt's offscreen platform, so it needs Qt Widgets and Qt Test
# but no display and no frontend build.
find_package(Qt6 QUIET COMPONENTS Widgets Test)
set(HAVE_QT6_TEST FALSE)
if(TARGET Qt6::Test)
  set(HAVE_QT6_TEST TRUE)
endif()
# Target and test are declared in ../build/test-graph.json.
