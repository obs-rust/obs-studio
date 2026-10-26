# Characterization of the raw output audio admission and the capture hookup of
# libobs/obs-output.c, moved to libobs/obs-output-raw-audio.c before their Rust
# port: prepare_audio()'s passthrough, wait, drop and front cut;
# default_raw_audio_callback()'s audio start, block and push sizes, block loop,
# block timestamps and frame count; start_raw_audio()'s mixes; and
# hook_data_capture()'s encoded callback and delay. prepare_audio(), the block
# loop, start_raw_audio() and hook_data_capture() are also copied onto a
# stand-in output, so the admission is pinned end to end against a video start
# at 48 kHz, the block loop's held-back block (characterized, not endorsed),
# every mixer mask with and without raw_audio2 and every flag combination of
# the hookup. Builds the moved file and obs-output-video-checks.c (for
# obs_output_first_mixer()) into itself. The functions are hidden in libobs,
# so this characterizes that C source; the test calls only them.
# test_raw_output_video_reset, test_output_before_video,
# test_output_delay_e2e, test_output_delay_restart,
# test_output_interleave_e2e, test_output_input_checks,
# test_replay_buffer_pause_stop and test_ffmpeg_output_drain cover the callers
# through the real libobs.
# Target and test are declared in ../build/test-graph.json.
