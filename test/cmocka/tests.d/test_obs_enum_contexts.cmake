# Characterization of obs_enum_outputs, obs_enum_encoders and
# obs_enum_services before their Rust swap (#5448): an empty list never calls
# back; every entry is visited once, newest first, with the caller's param;
# returning false stops the walk; the callback may enumerate the same kind
# again; an object it creates is not visited by the walk in progress.
# Registered once in build/test-graph.json.
