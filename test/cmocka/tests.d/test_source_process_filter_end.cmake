# obs_source_process_filter_end and obs_source_process_filter_tech_end
# (#5829), called directly and from a filter that renders through them: the
# NULL filter log line, the texture and bypass draws, the "Draw" default and
# an unknown technique, linear sRGB from the filter's flag, and the bypass
# flag cleared for a removed filter. Linux with OpenGL, under xvfb-run.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
