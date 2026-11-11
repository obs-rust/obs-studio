# A second obs_output_start() while the output is still starting (a stream
# still connecting) ran the output's start() again and crashed rtmp_output
# (obsproject/obs-studio#13283). No video or encoders; obs_startup() needs a
# display on Linux, so there the cases skip without one.
# Defined in ../build/test-graph.d.
