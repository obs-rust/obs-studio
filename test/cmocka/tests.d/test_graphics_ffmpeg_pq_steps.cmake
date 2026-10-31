# Characterization of the JPEG XR PQ-to-scRGB conversion of
# libobs/graphics/graphics-ffmpeg-host-windows.c (pq_to_linear() and
# convert_pq_to_cccs()), moved to libobs/graphics/graphics-ffmpeg-host-pq.c
# before its port to rust/obs-graphics (image_pq_scrgb, #3250): the inverse
# PQ curve at code values 0, 1, 2, 512, 1022 and 1023, half.h's
# half_from_float() at its edges, black, white, a grey ramp, each primary at
# full scale and at code value 1, bits 30-31, the texel order with alpha 1.0
# and an empty input. A texel outside BT.709 keeps its negative values:
# characterized, not endorsed. Values that depend on powf() are checked to
# within the last place. Builds the moved file on its own: no Windows header
# and no libobs core, so it runs on every platform. The function is hidden in
# libobs and built only on Windows, so this characterizes that C source; no
# real-libobs test loads a JPEG XR file. Target and test are declared in
# ../build/test-graph.json.
