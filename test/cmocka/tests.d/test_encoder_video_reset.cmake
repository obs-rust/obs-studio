# Video encoders follow obs_reset_video() instead of keeping the freed video_t
# (obsproject/obs-studio#12213). Needs core video, so it runs (under xvfb-run)
# on Linux and skips without a display.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
