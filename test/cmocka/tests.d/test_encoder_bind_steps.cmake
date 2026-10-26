# Characterization of the encoder media binding and shutdown checks of
# libobs/obs-encoder.c, moved to libobs/obs-encoder-bind.c before its Rust
# port: the audio type refusal (the video one is
# libobs/obs-encoder-audio-start.c's, which the test also builds), the video
# and audio timebases, the frame rate divisor video, and the trylock results
# and the shutdown decision of obs_encoder_shutdown_unstarted(). The bodies of
# both setters, encoder_set_video(), the three media getters, the unstarted
# shutdown and the unbind and rebind across a video reset are also copied onto
# a stand-in encoder, so every gate is pinned end to end for audio and video
# encoders, active and idle, initialized and not (the audio setter taking an
# initialized encoder: characterized, not endorsed). Builds the two files on
# their own: no display, no GPU and no thread, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. test_encoder_video_reset and
# test_output_failed_start_encoders cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
