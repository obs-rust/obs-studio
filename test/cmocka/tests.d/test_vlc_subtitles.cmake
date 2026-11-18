# VLC Video Source "Enable subtitles" setting (upstream obsproject/obs-studio#14199).
# The end-to-end cases load the built vlc-video module, need core video (run
# under xvfb-run) and skip without a display or when libVLC is not installed;
# the test then exits with 77 and ctest reports it as skipped.
# Defined in ../build/test-graph.json.
