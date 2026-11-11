# obs-outputs: the null output joins every stop thread it starts, also one
# that finished before the next start (split from obs-rust/obs-studio#1464).
# The real null-output.c is compiled in with libobs's output calls and
# pthread_create/pthread_join wrapped (GNU ld --wrap, so Linux only), so no
# encoder or display is needed.
# Defined in ../build/test-graph.d.
