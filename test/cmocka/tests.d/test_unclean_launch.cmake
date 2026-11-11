# After an unclean shutdown, --disable-shutdown-check must skip the modal
# "Crash Detected" dialog, which blocks unattended starts forever
# (obs-rust/obs-studio#809). Builds the Qt-free header OBSApp uses.
# Target and test are declared in ../build/test-graph.d.
