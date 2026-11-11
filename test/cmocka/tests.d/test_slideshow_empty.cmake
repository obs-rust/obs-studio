# Legacy slideshow callbacks use the original image/transition owners.
set(HAVE_SLIDESHOW_TEST_OWNER FALSE)
if(TARGET image-source AND TARGET obs-transitions)
  set(HAVE_SLIDESHOW_TEST_OWNER TRUE)
endif()
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
