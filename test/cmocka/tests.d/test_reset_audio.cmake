# Characterization of obs_reset_audio before its Rust swap (#5479): the fields
# it does not take fall back to the obs_reset_audio2 defaults, each call logs
# one reset message, and without a core it returns false. Audio only, no
# display needed. Registered once in build/test-graph.json.
