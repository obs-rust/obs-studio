# Characterization of the 16-bit RGBA rows of libobs/graphics/graphics-ffmpeg.c
# (the AV_PIX_FMT_RGBA64BE branch of ffmpeg_image_reformat_frame(), which 16-bit
# PNGs take), moved to libobs/graphics/graphics-ffmpeg-rgba64.c before its port
# to rust/obs-graphics (image_rgba64, #3242): each alpha mode on the edge
# channel values and the 8-bit rounding midpoint, at alphas 0, one half and 1,
# the channel and byte order, 1 x 1, odd widths and padded line sizes. A line
# size shorter than cx * 8 reads the source as one run and packs the
# rows and leaves the end of the output unwritten, and an alpha mode outside the
# three writes nothing: characterized, not endorsed. Builds the moved file on
# its own: no FFmpeg header and no libobs core, so it runs on every platform.
# The function is hidden in libobs, so this characterizes that C source; the
# test calls only it. test_image_file and the other image tests cover the
# caller through the real libobs, with no 16-bit fixture. Target and test are
# declared in ../build/test-graph.json.
