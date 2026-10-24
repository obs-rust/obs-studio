# Characterization of the clock and fit decisions of the transitions in
# libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-clock.c before its port: calc_time() around
# the start and the end, with a duration of 0 and of 2^63 and over, and in
# manual mode; the manual step of obs_transition_tick() with a torque of 0,
# with a clamp and with NaN targets (calc_torquef() as the tick calls it);
# get_cx(), get_cy() and the size fold; the scale of
# recalculate_transition_matrix() for each scale type, with a child smaller
# than, equal to and larger than the transition, a transition of size 0, a
# child of size 0 and scale types outside the enum; mix_spaces() over every
# pair; transition_calc_min_ts() with no, one and two timestamps. Builds the
# moved file into itself. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them and calc_torquef().
# test_transition_swap, test_studio_mode_swap,
# test_transition_recursive_destination, test_fade_to_color_mix,
# test_transition_audio_rescale and test_stinger_media_duration cover the
# callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
