# A delayed output that stopped itself without a reconnect (a lost connection
# with auto-reconnect off, an encoder error) did not start normally again: no
# "start" signal, and delayed packets reached it before it connected. Real
# libobs with core video, so it runs under xvfb-run on Linux and skips without
# a display.
# Defined in ../build/test-graph.d.
