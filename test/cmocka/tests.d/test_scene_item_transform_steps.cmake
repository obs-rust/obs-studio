# Characterization of the scene item transform composition of
# compute_item_transform() in libobs/obs-scene.c, moved to
# libobs/obs-scene-item-transform.c before its port: the bounds crop reset,
# a nested scene's crop rescaled by its size change (last_width 0, a size
# change, the same size, truncation, saturation), the cropped size, the
# bounds test, the box scale and both matrices (every alignment and two
# with extra bits, each bounds type, crop on and off, absolute and
# relative, the four scale signs, rotations 0, 90, 45, -30 and 720), run in
# compute_item_transform()'s order. Flipped items are pinned through
# x86-64's float-to-uint32_t cast, done in defined C, so the pins hold on
# every architecture. Builds the moved file into itself, with the hidden
# crop, coordinate and bounds files it is composed with; matrix4 comes from
# libobs. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The real-libobs scene, group and
# clipboard tests cover the caller.
# Target and test are declared in ../build/test-graph.json.
