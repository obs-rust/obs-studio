# VLC Video Source playlist track boundaries (upstream obsproject/obs-studio#9345).
# The end-to-end cases load the built vlc-video module, need core video (run
# under xvfb-run) and skip without a display or when libVLC is not installed;
# the test then exits with 77 and ctest reports it as skipped.
# A disabled module (ENABLE_VLC=OFF) still leaves a dummy custom target named
# vlc-video behind, which has no file, so check the target type too.
# Defined in ../build/test-graph.d.
