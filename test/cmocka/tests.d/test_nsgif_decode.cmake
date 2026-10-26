# The vendored libnsgif decoded directly, without libobs: frame 0 of each GIF
# against its reference PPM (#2465). A one-frame GIF goes to FFmpeg in
# gs_image_file, so test_image_file does not reach libnsgif for it.
# test_nsgif_decode is defined in ../build/test-graph.json.
