# The unchanged C characterization and its Rust-static variant are declared
# in ../build/test-graph.json. The Rust archive precedes shared libobs so the
# fixture exercises the Rust exports. Compile the real C preprocessor locally:
# its cleanup must use the same allocator as header-inline parser setup on macOS.
