# Characterization of the output stream delay in libobs/obs-output-delay.c
# before its port (#1546). process_delay() and the delay start, stop and
# cleanup are not exported, so the test compiles the file, with a zeroed
# struct obs_output, stubs for the obs-output.c functions it calls and a test
# clock: no libobs core and no display, so it runs on every platform. The
# file's precompiled header is libobs-ported-output-delay.pch.h (obs-internal.h),
# which needs the private libobs, libcaption, uthash and zlib headers.
# output_delay_fields_stub.c provides the delay field accessors of
# obs-output.c over the test's struct obs_output.
# test_output_delay is defined in ../build/test-graph.d.
