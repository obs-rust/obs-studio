# VLC Video Source hands its audio over as it plays, not in one-second bursts
# (upstream obsproject/obs-studio#2406). Generates Opus files with FFmpeg and
# plays them through the built vlc-video module. The cases need core video
# (run under xvfb-run) and skip without a display or when libVLC is not
# installed; the test then exits with 77 and ctest reports it as skipped.
# A disabled module (ENABLE_VLC=OFF) still leaves a dummy custom target named
# vlc-video behind, which has no file, so check the target type too.
# Defined in ../build/test-graph.d.
