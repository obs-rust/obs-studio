# Characterization of the transition video render of
# libobs/obs-source-transition.c before it moves to its own file and is ported
# to rust/obs-source-seam: obs_transition_video_render2()'s draw for each state
# (idle, transitioning with and without a callback, the texture lock busy), the
# placeholder for an empty slot, the textures' format following the color
# space, its video stop, obs_transition_video_render_direct()'s child for each
# target and its stop of a transition that is not active, and
# obs_transition_video_get_color_space()'s mix of the slots, through the real
# libobs. Transitions need core video, so it builds on Linux only and runs
# under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
