# obs-rust/obs-studio#2127: obs_output_set_mixer() with an index out of range,
# and obs_output_get_video_conversion() of an output with a scaled size but no
# video, through the real libobs on real core video (xvfb-run on Linux). ASan
# and UBSan cover the test's own code and libobs's frees; leak checks are off
# because the GL driver keeps an allocation from a module it unloads before
# exit, as Qt and fontconfig do in test_properties_dialog_refresh.
# Defined in ../build/test-graph.json.
