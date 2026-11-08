# Characterization of the scene item order, reorder and group membership
# decisions of libobs/obs-scene.c, moved to libobs/obs-scene-item-order.c
# before their port: the anchor of each movement, the position's bottom test
# and walk length, the item check, the match step of
# obs_scene_reorder_items(), the pair check and the group runs of
# obs_scene_reorder_items2(), the group add and remove gates, and the
# active-child gate of move_active_child(). Builds the
# moved file on its own: no display and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The real-libobs scene and group tests
# cover the caller.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the ten hidden functions, in the archive's obs-scene member,
# which also contains public getters with a logging dependency. The link-only
# logger aborts if these pure-rule tests unexpectedly reach it. OBS::libobs
# supplies obs.h's include
# directories), as test_scene_item_names_steps_rust does. Linking the archive
# directly avoids exporting private symbols just for a test.
# Rust-static target and test are also declared in ../build/test-graph.json.
