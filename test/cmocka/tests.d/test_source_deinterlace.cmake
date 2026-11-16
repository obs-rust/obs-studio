# The deinterlace of libobs/obs-source-deinterlace.c on a started core with
# video on libobs-opengl: the mode and field-order setters (the mode, the
# effect and the previous textures after each change), the frame take of a
# buffered and an unbuffered 2x source and after a time jump, the texture
# update after a render, and the render skip with no frame. Linux only, under
# xvfb-run; it skips without a display. Target and test are declared in
# ../build/test-graph.json.
