# Characterization of the pixel route of libobs/graphics/graphics-ffmpeg.c
# (ffmpeg_image_reformat_frame()'s choice between the straight copy, the
# premultiplied rows, the 16-bit rows and swscale, the format swscale is given
# and its range, the format afterwards, convert_format() and the straight row
# copy), moved to libobs/graphics/graphics-ffmpeg-route.c before its port to
# rust/obs-graphics (image_route, #3396). A source line size shorter than
# cx * 4 leaves the end of each row unwritten: characterized, not endorsed.
# Builds the moved file on its own: no FFmpeg header and no libobs core, so it
# runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. test_image_file and
# the other image tests cover the caller through the real libobs. Target and
# test are declared in ../build/test-graph.json.
