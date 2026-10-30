# Characterization of the caption plan of add_caption() in
# libobs/obs-output.c, moved to libobs/obs-output-caption.c before its port
# to rust/obs-output (output_caption, #3074): the carrier each codec gets
# (h264, av1, hevc with and without ENABLE_HEVC, near-miss names), the
# priority gate (INT_MIN to INT_MAX), the HEVC suffix SEI header after a
# 3-byte or 4-byte start code (the 3-byte one tried first) and no caption
# without one, and the CEA-608 triplet filter (cc_type, padding, zero,
# parity, and all 2^24 triplets). An HEVC packet shorter than its start code
# and header is not covered. Builds the moved file on its own: no display
# and no libobs core calls, so it runs on every platform. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. The Tier 3 parity tests in rust/obs-output compare the shims with
# this C.
# Target and test are declared in ../build/test-graph.json.
