# Characterization of the per-frame and per-encoder gates of the GPU encode
# thread (libobs/obs-video-gpu-encode.c), moved to
# libobs/obs-encoder-gpu-gates.c and ported to rust/obs-output
# (encoder_gpu_gates): the start-up wait, the frame rate divisor counter, the
# keyed mutex key each encoder hands on, the frame timing entry, the requeue
# and init_gpu_encoding()'s texture format, with the gates the thread shares
# with receive_video() (libobs/obs-encoder-audio-start.c: the group start, the
# paired audio wait and the pts step). The per-frame body of
# gpu_encode_thread() is also copied onto stand-in encoders, so divisors 1, 2
# and 3, the keys when every encoder encodes and when one skips
# (characterized, not endorsed), the group and paired waits, the pts after a
# failed encode and the requeue near UINT64_MAX are pinned end to end. Builds
# the two moved files on their own: no display, no GPU and no thread, so it
# runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member also holds the
# rest of obs-output (the service lifecycle, the delay and the interleaver),
# which needs libobs's hidden output and service seams and obs-util, so it does
# not link on its own, as for test_encoder_audio_start_steps. The Tier 3 parity
# tests in rust/obs-output compare the shims with this C instead.
# test_encoder_error_stop, test_texture_encoder_raw_format and
# test_encoder_join_keyframe cover the thread through the real libobs.
# Target and test are declared in ../build/test-graph.json.
