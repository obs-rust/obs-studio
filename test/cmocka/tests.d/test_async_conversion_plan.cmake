# Characterization of the async video GPU conversion plan of
# libobs/obs-source.c (get_convert_type(), the set_*_sizes() of
# init_gpu_conversion(), select_conversion_technique() and
# need_linear_output()), moved to libobs/obs-source-conversion-plan.c before
# its port: the whole table, every video format and two values outside the
# enum, both ranges, every transfer and two values outside the enum, at sizes
# 0 to 3, odd, 1920 x 1080 and the uint32_t wraps near UINT32_MAX. Builds the
# moved file into itself. The function is hidden in libobs, so this
# characterizes that C source; the test calls only it.
# test_media_restart_first_frame, test_deinterlace_2x_field_order and
# test_pipewire_camera_nv12 cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.
