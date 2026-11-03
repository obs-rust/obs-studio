# Characterization of the effect parser's struct and sampler-state blocks
# (#3550): ep_parse() on an effect's text, then the structs, samplers, params
# and functions it kept, the parse errors and the allocations still held after
# ep_free(), written out and compared whole, for well-formed blocks, malformed
# ones and text cut short inside each block. ep_parse() is hidden in libobs, so
# the test compiles the original effect parser C into itself. No display and
# no graphics context, so it runs on every platform. Target and test are
# declared in ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). The port has no
# intentional difference, so the test file is the same. ep_parse() stays C
# and is compiled in as above; effect-parser-blocks.c is not, so its two
# blocks, ep_free() and the parameter type decisions, all hidden in the
# shared library, come from the exact production staticlib at executable link
# time. The staticlib's obs-graphics calls the private obs-graphics and effect
# parser adapters, which libobs links but does not export, so this executable
# compiles its own copies, as test_effect_param_types_steps_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_effect_parser_blocks_rust
    test_effect_parser_blocks.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_effect_parser_blocks_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(
    test_effect_parser_blocks_rust
    PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_effect_parser_blocks_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_effect_parser_blocks_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_parser_blocks_rust)
  add_test(test_effect_parser_blocks_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_blocks_rust)
endif()
