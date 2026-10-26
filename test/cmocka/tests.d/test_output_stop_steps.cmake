# Characterization of the output stop and end-capture decisions of
# libobs/obs-output.c, moved to libobs/obs-output-stop.c and ported to
# rust/obs-output (output_stop):
# the encoded flag (the delayed stop, the encoders or the raw connections, the
# unstarted encoders' shutdown), a running delay (the delayed stop, the
# process_delay() callback, the delay cleanup), which encoders stop, the
# service deactivation, whether restart references keep the delay, the frame
# log and the thread creation fallback; with the two rules the callers share
# with ported files, the interleave test and each mix's bit
# (libobs/obs-output-raw-audio.c) and the first mixer
# (libobs/obs-output-video-checks.c), which the test also builds. The bodies
# of obs_output_stop(), obs_output_force_stop(), obs_output_actual_stop(),
# stop_raw_audio(), end_data_capture_thread(),
# shutdown_unstarted_encoders_thread(), obs_output_end_data_capture_internal(),
# obs_output_end_data_capture() and obs_output_signal_stop() are also copied
# onto a stand-in output that records every call, event, signal and warning,
# so every route over the atomics (active, data_active, reconnecting,
# delay_active, delay_capturing, the end-capture thread's flag and the
# stopping event) is pinned end to end, with the characterized-not-endorsed
# cases (a negative restart count keeping the delay, a raw output cleaning up
# a delay). Builds the three files on their own: no display, no GPU and no
# thread, so it runs on every platform. The functions are hidden in libobs,
# so this characterizes that C source; the test calls only them. There is
# no _rust variant against libobs_rust-static: the shims' archive member also
# holds the rest of obs-output, which needs libobs's hidden output and service
# seams and obs-util, so it does not link on its own, as for
# test_output_start_steps. The Tier 3 parity tests in rust/obs-output compare
# the shims with this C instead.
# test_output_end_capture_thread_join, test_output_deactivate_state,
# test_null_output_stop_thread, test_ffmpeg_output_stop,
# test_ffmpeg_output_drain, test_encoder_error_stop,
# test_replay_buffer_pause_stop, test_output_delay_e2e,
# test_output_failed_start_encoders, test_recording_shutdown and
# test_pthread_init_failures cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
