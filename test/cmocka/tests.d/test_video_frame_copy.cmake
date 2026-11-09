# Characterization of the output frame copy of libobs/obs-video.c
# (set_gpu_converted_plane(), set_gpu_converted_data() and copy_rgbx_frame())
# before it moves to its own file and is ported to rust/obs-core: each plane's
# line size and samples as a raw video callback gets them, for every format
# with a GPU conversion and the 4-byte RGB formats, with the conversion on and
# off, at a size where the rows match and one where they don't, through the
# real libobs. Core video needs a graphics module, so it builds on Linux only
# and runs under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
