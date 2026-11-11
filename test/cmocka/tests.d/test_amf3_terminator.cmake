# AMF3 dynamic-member terminator handling, through librtmp's decoder under
# ASan: on master the empty-name terminator is appended as a property and the
# stale union pointer from the preceding member is then freed twice.
# Defined in ../build/test-graph.d.
