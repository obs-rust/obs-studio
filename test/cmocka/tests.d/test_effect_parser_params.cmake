# Characterization of the effect parser's parameter value assignments
# (#3610): ep_parse() on an effect's text, then the params it kept (flags,
# array count and every byte of the default value), the functions after them,
# the parse errors and the allocations still held after ep_free(), written out
# and compared whole, for each value form, malformed values and text cut short
# at each step. ep_parse() is hidden in libobs, so the test compiles the
# original effect parser C into itself. No display and no graphics context, so
# it runs on every platform. Target and test are declared in
# ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). The port has no
# intentional difference, so the test file is the same. ep_parse(), the
# function parser and the shader writer it calls (effect-parser-funcs.c and
# effect-parser-writer.c) stay C and are compiled in as above;
# effect-parser-params.c, effect-parser-blocks.c
# and effect-parser-passes.c are not, so the parameter declarations, the two
# blocks, the techniques, ep_free() and the parameter type decisions, all
# hidden in the shared library, come from the exact production staticlib at
# executable link time.
# The staticlib's obs-graphics calls the private obs-graphics and effect
# parser adapters, which libobs links but does not export, so this executable
# compiles its own copies, as test_effect_parser_passes_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_effect_parser_params_rust
    test_effect_parser_params.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-funcs.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-writer.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_effect_parser_params_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  if(MSVC)
    # Keep this public lookup in obs.dll. A plain call loads its import thunk
    # before the private shims pull a Rust object defining the same symbol.
    # The test-only bridge calls the import address instead (#3603).
    target_sources(test_effect_parser_params_rust PRIVATE hosts/windows/effect-parser-lookup-import.c)
    target_compile_definitions(
      test_effect_parser_params_rust
      PRIVATE gs_effect_get_param_by_name=obs_test_effect_get_param_by_name
    )
    # Import only the address slot, without obs.lib's function thunk. DATA
    # describes the import library entry, not the DLL export's implementation.
    set(lookup_import "${CMAKE_CURRENT_BINARY_DIR}/effect-parser-params-lookup-import.lib")
    add_custom_command(
      OUTPUT "${lookup_import}"
      COMMAND
        "${CMAKE_AR}" /nologo "/def:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows/effect-parser-lookup-import.def"
        "/machine:${CMAKE_C_COMPILER_ARCHITECTURE_ID}" "/out:${lookup_import}"
      DEPENDS hosts/windows/effect-parser-lookup-import.def
      VERBATIM
    )
    target_sources(test_effect_parser_params_rust PRIVATE "${lookup_import}")
    # Resolve the address slot before the ordinary libobs import library.
    target_link_libraries(test_effect_parser_params_rust PRIVATE "${lookup_import}")
  endif()
  target_link_libraries(
    test_effect_parser_params_rust
    PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_effect_parser_params_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_effect_parser_params_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_parser_params_rust)
  add_test(test_effect_parser_params_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_params_rust)
endif()
