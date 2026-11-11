# The RTMP send thread leaks the packet it is sending when the connection
# fails (upstream obsproject/obs-studio#9972). Builds the real rtmp-stream.c
# (with the obs-outputs sources it links against) into the test and runs its
# send thread on a socket pair; the output and encoder are fakes, so no
# network, display or encoder is needed.
# Defined in ../build/test-graph.d.
