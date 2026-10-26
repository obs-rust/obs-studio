# Characterization of the encoder slot rules and setter gates of
# libobs/obs-output.c, moved to libobs/obs-output-slots.c before their Rust
# port: the first occupied video and audio slot, set_mixer()'s range and mask,
# the encoder type gates (also the slot choice of
# obs_output_remove_encoder_internal()), the two setters' index rules (with and
# without the multitrack flags), the default index, the preferred size step and
# the getters' bounds. The callers (both encoder setters, the getters,
# obs_output_audio(), set_mixer(), remove_encoder_internal() and
# set_preferred_size2()) are also copied onto a stand-in output that records
# every warning, active load, reference, hookup, caption track and scaled size,
# so the gates are pinned end to end for raw, encoded, video-only, audio-only
# and multitrack outputs, with the characterized-not-endorsed cases (a caption
# track for a slot whose reference failed, the scaled size set on a cleared
# slot). Builds the moved file into itself. The functions are hidden in libobs,
# so this characterizes that C source; the test calls only them.
# test_multitrack_encoder_override, test_multitrack_ladder,
# test_output_input_checks, test_service_encoder_settings,
# test_output_interleave_e2e, test_output_caption_codec,
# test_raw_output_video_reset and test_output_null_id cover the callers through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.
