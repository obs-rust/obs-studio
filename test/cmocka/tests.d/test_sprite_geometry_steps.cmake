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
# Target and test are declared in ../build/test-graph.json.
