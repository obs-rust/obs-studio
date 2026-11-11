# An Image Slide Show in Manual mode with Playback Mode "Once" ends on its
# last slide instead of wrapping to the first on Next (forum thread 138392).
# Builds both slideshows (obs-slideshow-mk2.c, obs-slideshow.c) and the Cut
# transition directly so the image-source and obs-transitions modules are not
# loaded. Needs core video, so it runs (under xvfb-run) on Linux and skips
# without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
