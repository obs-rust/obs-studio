# Placement half of calculate_bounds_data (alignment, crop, flip).
# The C-reference target/test are declared in ../build/test-graph.d.

# Exercise the unchanged characterization against the Rust internal symbols.
# The regular executable above remains the original C reference. Linking the
# archive directly avoids exporting private symbols just for a test.
# Rust-static target and test are also declared in ../build/test-graph.d.
