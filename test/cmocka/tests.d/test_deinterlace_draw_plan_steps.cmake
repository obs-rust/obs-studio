# Characterization of the deinterlace draw decisions of
# libobs/obs-source-deinterlace.c (get_effect(), deinterlace_linear_required(),
# deinterlace_render() and obs_source_set_deinterlace_mode()), moved to
# libobs/obs-source-deinterlace-draw.c: the effect and the linear rule of every
# mode and of modes outside the enum, the early return, the technique and
# multiplier of every pair of colour spaces at SDR white levels of 80, 300 and
# 0, the second field at its threshold and where the uint64_t sum wraps, and
# every mode change (#2120). Builds the moved file into itself. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them. test_deinterlace_2x_field_order and test_deinterlace_mode_load
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
