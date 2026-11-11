# Characterization of the encoded packet interleaver in
# libobs/obs-output-interleave.c before its port (#1594). The interleaver's
# functions are static, so the test compiles the file, with a zeroed struct
# obs_output and stubs for the obs-output.c functions it calls: no libobs core
# and no display, so it runs on every platform. The file's precompiled header
# is libobs-ported-output-interleave.pch.h (obs-internal.h), which needs the
# private libobs, libcaption, uthash and zlib headers.
# output_interleave_fields_stub.c provides the output, caption track and
# encoder accessors of obs-output.c and obs-encoder.c over the test's structs.
# test_output_interleave is defined in ../build/test-graph.d.
