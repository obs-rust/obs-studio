# Characterization of the output protocol-list match in
# libobs/obs-output-protocols.c, ported to rust/obs-output (protocols): how
# many ';'-separated segments of a service output's protocols string equal the
# protocol. Builds the moved file on its own: no display and no libobs core,
# so it runs on every platform. The function is hidden in libobs, so this
# characterizes that C source. There is no _rust variant against
# libobs_rust-static: the shim's archive member also holds the rest of
# obs-output (the delay, the interleaver and the caption text), which needs
# libobs's hidden output and encoder seams and obs-util (libuuid), so it does
# not link on its own. The Tier 3 parity tests in rust/obs-output compare the
# shim with this C instead. test_output_protocols covers the caller through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.
