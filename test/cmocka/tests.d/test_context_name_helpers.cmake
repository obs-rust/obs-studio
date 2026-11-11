# Characterization of the context name helpers in libobs/obs-context-names.c,
# ported to rust/obs-core (context_names): the "__unnamed%04lld" name and the
# " 2", " 3", ... suffix, against a scripted exists(). Builds the helpers on
# their own: no libobs core and no name tables. The functions are hidden in
# libobs, so this characterizes that C source. There is no _rust variant
# against libobs_rust-static: the shims' archive member also holds the rest
# of obs-core, which pulls in obs-media-io and obs-util (libuuid, GIO, and
# libobs's hidden base_default_log_handler), so it does not link on its own.
# The Tier 3 parity tests in rust/obs-core compare the shims with this C
# instead. test_context_names covers the callers through the real libobs.
# Defined in ../build/test-graph.d.
