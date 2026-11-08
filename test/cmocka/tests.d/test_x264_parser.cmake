# The x264 owner already consumed its one canonical entry during plugin setup.
# Do not register it twice in this later all-core graph pass.
set(HAVE_X264_TEST_OWNER FALSE)
if(TARGET obs-x264-test)
  add_dependencies(cmocka-tests obs-x264-test)
endif()
