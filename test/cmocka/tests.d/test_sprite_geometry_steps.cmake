# Characterization of the sprite and texture upload geometry of
# libobs/graphics/graphics.c, moved to libobs/graphics/graphics-sprite.c before
# its port: the points and texture coordinates of a sprite (normalized, a
# rectangle texture's texels, a subregion's, with every flip value and a
# 0-size texture's inf and NaN), the refusals and size choice of
# gs_draw_quadf(), the static buffer a flip draws, the whole-texture shortcut of
# gs_draw_sprite_subregion(), the copy plan of gs_texture_set_image() (equal and
# unequal line sizes, flipped, and a flip with a height of 0) and the
# power-of-two and mipmap rules of gs_texture_create(). Builds the moved file
# on its own: no display and no graphics device, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. test_texture_render, test_image_size_precheck, the
# test_graphics_alias_* tests and the scene render tests cover the callers
# through the real libobs.
# Target and test are declared in ../build/test-graph.d.

# Exercise the unchanged characterization against the Rust shims, built and
# run only with the hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target).
# The functions are hidden in the shared library, so the test selects them from
# the exact production staticlib at executable link time. The staticlib's
# obs-graphics also holds gs_image_file_ex_*, gs_texrender_* and the effect
# parser lifecycle, which call libobs exports (linked), libnsgif (in the
# staticlib, ported) and the private obs-graphics and effect parser adapters;
# libobs links those
# but does not export them, so this executable compiles its own copies, as
# test_effect_parser_lifecycle_rust does. graphics-sprite.c is not compiled in.
# Rust-static target and test are also declared in ../build/test-graph.d.
