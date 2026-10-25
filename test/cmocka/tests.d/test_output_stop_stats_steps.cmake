# Characterization of the output stop statistics of log_frame_info() in
# libobs/obs-output.c, moved to libobs/obs-output-stop-stats.c and ported to
# rust/obs-output (stop_stats): the frames drawn and lagged since the output
# started (uint32_t differences, wrapped counters included), the two double
# percentages bit for bit, and which of the lines after "stopping" are logged
# with which numbers (the attempted forms of "Total frames output" and "Total
# drawn frames", the lag line and the drop line), over zero counters, wrapped
# counters, and the plugin's dropped count negative, zero, equal to and above
# total, INT_MIN and INT_MAX. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The function is hidden in
# libobs, so this characterizes that C source; the test calls only it. There is
# no _rust variant against libobs_rust-static: the shim's archive member also
# holds the rest of obs-output (the service lifecycle, the delay and the
# interleaver), which needs libobs's hidden output and service seams and
# obs-util, so it does not link on its own, as for test_encoder_size_steps. The
# Tier 3 parity tests in rust/obs-output compare the shim with this C instead.
# test_encoder_error_stop, test_encoder_video_reset, test_encoding_lag_rescale,
# test_output_before_video, test_output_delay_restart,
# test_output_end_capture_thread_join, test_output_interleave_e2e,
# test_output_interleave_restart, test_raw_output_video_reset,
# test_replay_buffer_pause_stop, test_scaled_mix_alpha and
# test_texture_encoder_raw_format stop video outputs, so they run
# log_frame_info() through the real libobs; none checks its lines.
# Target and test are declared in ../build/test-graph.json.
