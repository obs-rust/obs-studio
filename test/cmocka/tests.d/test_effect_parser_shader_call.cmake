# test_effect_parser_shader_call (#3625) and test_effect_parser_writer (#3765)
# compile the original effect parser C into themselves, so they characterize
# the C writer (libobs/graphics/effect-parser-writer.c). Targets and tests are
# declared in ../build/test-graph.json.
#
# test_effect_parser_shader_call again against the Rust writer, built and run
# only with the hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target), with
# the same file and expectations: ep_parse() runs ep_compile() through the two
# renamed shader-create recorders, so the shader text it checks is the one
# ep_makeshaderstring() writes. ep_parse(), the compile step and the function
# parser (effect-parser.c, effect-parser-compile.c and effect-parser-funcs.c)
# stay C and are compiled in; the writer, the blocks, the params, the passes, ep_free() and the
# parameter type decisions, all hidden in the shared library, come from the
# exact production staticlib at executable link time. The staticlib's
# obs-graphics calls the private obs-graphics and effect parser adapters,
# which libobs links but does not export, so this executable compiles its own
# copies, as test_effect_parser_passes_rust does.
#
# There is no test_effect_parser_writer_rust: the port's intentional
# difference (#3768, main() puts commas between its mapped params) changes
# the text that test pins for two of its cases. The Tier 1 tests
# (rust/obs-graphics/tests/shader/effect_parser_writer.rs) port its cases
# one to one with that difference, and the Tier 3 parity tests
# (effect_parser_writer_parity.rs) compare the shim with this C.
if(TARGET libobs_rust-static)
  add_executable(
    test_effect_parser_shader_call_rust
    test_effect_parser_shader_call.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-compile.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-funcs.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_effect_parser_shader_call_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_compile_definitions(
    test_effect_parser_shader_call_rust
    PRIVATE gs_pixelshader_create=test_pixelshader_create gs_vertexshader_create=test_vertexshader_create
  )
  if(MSVC)
    # Keep this public lookup in obs.dll. A plain call loads its import thunk
    # before the private shims pull a Rust object defining the same symbol.
    # The test-only bridge calls the import address instead (#3603).
    target_sources(test_effect_parser_shader_call_rust PRIVATE hosts/windows/effect-parser-lookup-import.c)
    target_compile_definitions(
      test_effect_parser_shader_call_rust
      PRIVATE gs_effect_get_param_by_name=obs_test_effect_get_param_by_name
    )
    # Import only the address slot, without obs.lib's function thunk. DATA
    # describes the import library entry, not the DLL export's implementation.
    set(lookup_import "${CMAKE_CURRENT_BINARY_DIR}/effect-parser-shader-call-lookup-import.lib")
    add_custom_command(
      OUTPUT "${lookup_import}"
      COMMAND
        "${CMAKE_AR}" /nologo "/def:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows/effect-parser-lookup-import.def"
        "/machine:${CMAKE_C_COMPILER_ARCHITECTURE_ID}" "/out:${lookup_import}"
      DEPENDS hosts/windows/effect-parser-lookup-import.def
      VERBATIM
    )
    target_sources(test_effect_parser_shader_call_rust PRIVATE "${lookup_import}")
    # Resolve the address slot before the ordinary libobs import library.
    target_link_libraries(test_effect_parser_shader_call_rust PRIVATE "${lookup_import}")
  endif()
  target_link_libraries(
    test_effect_parser_shader_call_rust
    PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_effect_parser_shader_call_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_effect_parser_shader_call_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_parser_shader_call_rust)
  add_test(test_effect_parser_shader_call_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_shader_call_rust)
endif()
