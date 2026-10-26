# Characterization of the output start and begin-capture decisions of
# libobs/obs-output.c, moved to libobs/obs-output-start.c and ported to
# rust/obs-output (output_start): the service flag (the gate before the start
# and the activation after the hookup), the delayed restart's reference, which
# encoders are initialized, the raw reset and the encoder pairing; with the two
# rules the callers share with ported files, the delayed start
# (libobs/obs-output-raw-audio.c) and the audio conversion fill
# (libobs/obs-encoder-size.c), which the test also builds. The bodies of
# obs_output_start(), obs_output_actual_start(),
# obs_output_initialize_encoders(), begin_delayed_capture(),
# reset_raw_output(), obs_output_begin_data_capture_internal() and
# obs_output_begin_data_capture() are also copied onto a stand-in output that
# records every call, store, signal and warning, so every route over the
# atomics (starting, delay_active, active, reconnecting, delay_capturing) is
# pinned end to end, with the characterized-not-endorsed cases (a negative
# restart count dropped, a raw output delayed or paired). Builds the three
# files on their own: no display, no GPU and no thread, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive member also holds the rest of
# obs-output, which needs libobs's hidden output and service seams and
# obs-util, so it does not link on its own, as for test_output_slots_steps.
# The Tier 3 parity tests in rust/obs-output compare the shims with this C
# instead. test_output_start_twice,
# test_output_failed_start_encoders, test_output_delay,
# test_output_delay_restart, test_output_before_video,
# test_raw_output_video_reset, test_output_interleave_restart and
# test_rtmp_send_thread_leak cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
