# test_effect_parser_compile (#3792) compiles the original effect parser C
# into itself, so it characterizes the C compile step
# (libobs/graphics/effect-parser-compile.c). Its target and test are declared
# in ../build/test-graph.json.
#
# The same cases against the Rust compile step, built and run only with the
# hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target), with the same
# file and expectations: the port has no intentional difference these texts
# reach. ep_parse() and the function parser (effect-parser.c and
# effect-parser-funcs.c) stay C and are compiled in; the compile step, the
# shader writer, the blocks, the params, the passes, ep_free() and the
# parameter type decisions, all hidden in the shared library, come from the
# exact production staticlib at executable link time. The staticlib's
# obs-graphics calls the private obs-graphics and effect parser adapters,
# which libobs links but does not export, so this executable compiles its own
# copies, as test_effect_parser_shader_call_rust does. The compile step
# creates its shaders and finds their params through those adapters, so the
# same four renames as test-graph.json's put the test's stand-ins behind them.
if(TARGET libobs_rust-static)
  add_executable(
    test_effect_parser_compile_rust
    test_effect_parser_compile.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-funcs.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_effect_parser_compile_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_compile_definitions(
    test_effect_parser_compile_rust
    PRIVATE
      gs_pixelshader_create=test_pixelshader_create
      gs_shader_destroy=test_shader_destroy
      gs_shader_get_param_by_name=test_shader_get_param_by_name
      gs_vertexshader_create=test_vertexshader_create
  )
  target_link_libraries(
    test_effect_parser_compile_rust
    PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_effect_parser_compile_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_effect_parser_compile_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_parser_compile_rust)
  add_test(test_effect_parser_compile_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_compile_rust)
endif()
