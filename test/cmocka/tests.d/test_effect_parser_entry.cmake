# The entry-point characterization uses the real C parser and a recording
# compile step. Its sources and dependencies are in ../build/test-graph.json.
# ep_parse() is ported to rust/obs-graphics (effect_parser::entry), and this
# test keeps characterizing the original C. There is no _rust variant against
# libobs_rust-static: the test defines its own recording ep_compile(), which
# the archive also defines, and the Rust ep_parse() calls the Rust compile
# step directly, so the stand-in would not see its calls. The Rust
# ep_parse() runs instead in the _rust variants of test_effect_parser_blocks,
# _params, _passes, _compile and _shader_call, and the Tier 3 parity tests in
# rust/obs-graphics compare it with this C.
