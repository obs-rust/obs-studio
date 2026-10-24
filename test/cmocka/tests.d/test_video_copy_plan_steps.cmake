# Characterization of the output frame copy plan and the scale effect choice of
# libobs/obs-video.c, moved to libobs/obs-video-copy-plan.c and ported to
# rust/obs-core (video_copy_plan): the copies set_gpu_converted_data() makes for
# every format (each plane's row size, rows and strides, one copy or rows, the
# packed NV12 and P010 UV plane), copy_rgbx_frame()'s copy, resolution_close()
# at +-15, 16 and 17, the scale effect for each scale type, low resolution or
# not, close or not and each effect missing, and one comparison of
# can_reuse_mix_texture(). Widths near the uint32_t wrap are characterized, not
# endorsed. Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member (an obs_core
# codegen unit) also holds the context names, the module locale and source
# rules, the audio mix and the source profiler, which need libobs (blog,
# bmalloc, the graphics timers, obs_queue_task) and pull in the rest, so it does
# not link on its own (checked: 404 undefined references). The Tier 3 parity
# tests in rust/obs-core compare the shims with this C instead.
# test_video_output_formats, test_scaled_mix_alpha, test_encoding_lag_rescale
# and test_video_output_plan cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
