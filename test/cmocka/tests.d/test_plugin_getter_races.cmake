# Registered once in ../build/test-graph.json for both native consumers.
# Retain the original conjunction of all four owning targets on every host.
set(HAVE_PLUGIN_GETTER_OWNERS FALSE)
if(TARGET obs-filters AND TARGET image-source AND TARGET obs-transitions AND TARGET linux-capture)
  set(HAVE_PLUGIN_GETTER_OWNERS TRUE)
endif()
