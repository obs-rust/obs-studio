# Free disk space is queried off the UI thread (obs-rust/obs-studio#1282):
# request() and latest() return at once while the free-space query blocks.
# Builds the Qt-free helper the recording timer and the stats window call.
# Target and test are declared in ../build/test-graph.d.
