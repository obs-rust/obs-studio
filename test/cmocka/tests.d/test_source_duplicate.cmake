# obs_source_duplicate (#5805): a NULL source logs the validity check's line
# once; a public scene and a do-not-duplicate source are shared with a new
# reference; a private scene and any group are copied with
# obs_scene_duplicate(); any other source is created anew (public or
# private) from a copy of its settings, with its audio state, flags, private
# settings and filters copied. Real core under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
