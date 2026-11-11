# AMF3 class-name and member-name logging, through librtmp's decoder under
# ASan: both names are length-prefixed AVals, and logging one with %s reads
# past the allocation end.
# Defined in ../build/test-graph.d.
