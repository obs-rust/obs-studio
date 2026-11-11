# dstr.c compiled into the test with UBSan aborting on the first report
# (obs-rust/obs-studio#174). The local definitions take precedence over
# libobs's exports on ELF; MSVC has no UBSan, so it is not built there.
# Defined in ../build/test-graph.d.
