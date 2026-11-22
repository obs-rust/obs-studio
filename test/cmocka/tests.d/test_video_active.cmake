# Characterization of obs_video_active before its Rust swap (#5434): false
# before the first video reset and on an idle mix, true for a raw callback on
# the main mix, a raw encoder on an auxiliary canvas and a texture encoder
# alone, and only for counts above zero; it changes neither count. Needs a
# display and OpenGL. Registered once in build/test-graph.json.
