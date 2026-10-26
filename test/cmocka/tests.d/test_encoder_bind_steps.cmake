# Characterization of the encoder media binding and shutdown checks of
# libobs/obs-encoder.c, moved to libobs/obs-encoder-bind.c and ported to
# rust/obs-output (encoder_bind): the audio type refusal (the video one is
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
# test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive member also holds the rest of
# obs-output, which needs libobs's hidden output and service seams and
# obs-util, so it does not link on its own, as for
# test_encoder_audio_start_steps. The Tier 3 parity tests in rust/obs-output
# compare the shims with this C instead. test_encoder_video_reset and
# test_output_failed_start_encoders cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
