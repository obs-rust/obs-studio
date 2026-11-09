# Characterization of the rest of libobs/obs-source-transition.c before it
# is ported to rust/obs-source-seam: obs_transition_audio_render() called
# directly on children whose audio timestamp, flags and buffers the test sets
# (the copy of slot A while idle, the mix of each valid child with its
# callback by mixer mask and channel, the place of a child's audio, each
# sample's time, the audio stop and the removed child it can leave in slot A)
# and obs_transition_init()'s state, through the real libobs. Transitions
# need core video, so it builds on Linux only and runs under xvfb-run;
# without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
