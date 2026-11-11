# Characterization of the scene item blend mode table in
# libobs/obs-scene-blend.c before its port (#1520). The table is static, so the
# test compiles its file: no graphics device and no libobs core, so it runs on
# every platform. The file's precompiled header is libobs.pch.h, which needs
# the private libobs, libcaption, uthash and zlib headers.
# test_scene_blend is defined in ../build/test-graph.d.
