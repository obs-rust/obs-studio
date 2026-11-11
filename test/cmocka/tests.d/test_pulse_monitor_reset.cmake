# Failed PulseAudio reset must preserve the live monitor's resource lifetime.
# Interpose external libpulse symbols in the executable, as test_pulse_default_id
# does; monitoring and the resampler are exercised in the production libobs.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
