# Characterization of the decisions of the immediate-mode vertex builder of
# libobs/graphics/graphics.c, moved to libobs/graphics/graphics-immediate.c
# before their port to rust/obs-graphics (immediate, #3132): the push gates
# (the cap at GS_IMMEDIATE_COUNT in immediate mode, the texture units), the
# draw count of gs_render_stop() (each shorter normal, colour or texture 0
# array cuts it, and each differing one logs) and the buffer of
# gs_render_save() (its vertex count and the texture arrays it frees). A
# longer array that logs is characterized, not endorsed, and so are the C's
# answers where the port differs on purpose (any texture unit, every vertex
# saved with a shorter array, no texture array freed). Builds the moved file
# on its own: no display and no graphics context, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The rendering tests cover the callers
# through the real libobs.
# Target and test are declared in ../build/test-graph.d.

# The same cases against the Rust shims (GS_IMMEDIATE_TEST_RUST), with the
# port's intentional differences in place of the C's, built and run only with
# the hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target). The functions
# are hidden in the shared library, so the test selects them from the exact
# production staticlib at executable link time. The staticlib's obs-graphics
# also holds gs_image_file_ex_*, gs_texrender_* and the effect parser
# lifecycle, which call libobs exports (linked), libnsgif (in the staticlib,
# ported) and the private obs-graphics and effect parser adapters; libobs links
# those but does not export them, so this executable compiles its own copies,
# as test_sprite_geometry_steps_rust does. graphics-immediate.c is not
# compiled in.
# Rust-static target and test are also declared in ../build/test-graph.d.
