# Generic output, encoder and service enumeration must keep the current
# context alive while the callback releases its owned reference, stop when the
# callback says so, drop its snapshot references afterwards, and skip encoders
# that are already being destroyed. The isolated child tests use POSIX fork.
# Defined in ../build/test-graph.d.
