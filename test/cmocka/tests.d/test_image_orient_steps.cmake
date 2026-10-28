# Characterization of the image orientation and size check of
# libobs/graphics/graphics-ffmpeg.c, moved to
# libobs/graphics/graphics-ffmpeg-orient.c before their port to
# rust/obs-graphics (image_orient, #2809): the EXIF orientation from every sign
# combination of the display matrix, which orientations turn the image and
# which swap its size, every pixel of the turned image for orientations 2 to 8
# on lines, odd sizes and sizes around the 16-pixel tile (against the EXIF
# meaning of each orientation), and the size check against the device's
# maximum texture size. An orientation outside 0 to 8 (from the file's
# metadata) writes every pixel to the first one: characterized, not endorsed.
# Builds the moved file on its own: no display and no libobs core, so it runs
# on every platform. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. test_image_exif_orientation,
# test_image_size_precheck, test_image_jpeg_range and test_graphics_ffmpeg_host
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
