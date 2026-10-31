# Characterization of the parameter type decisions of the effect parser,
# libobs/graphics/effect-parser.c, moved to
# libobs/graphics/effect-param-types.c before their port to rust/obs-graphics
# (effect_param_types, #3162): the shader parameter type of a type name, the
# default-value parser a parameter gets, and the value count of an int or
# float array type with its row and column errors. A name that takes the
# string or array parser but has an unknown type, and a bad digit that still
# counts, are characterized, not endorsed. Builds the moved file on its own: no
# display and no graphics context, so it runs on every platform. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them. The effect tests cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). The port has no
# intentional difference, so the test file is the same. The functions are
# hidden in the shared library, so the test selects them from the exact
# production staticlib at executable link time. The staticlib's obs-graphics
# also holds gs_image_file_ex_*, gs_texrender_* and the effect parser
# lifecycle, which call libobs exports (linked), libnsgif (in the staticlib,
# ported) and the private obs-graphics and effect parser adapters; libobs links
# those but does not export them, so this executable compiles its own copies,
# as test_graphics_immediate_steps_rust does. effect-param-types.c is not
# compiled in.
if(TARGET libobs_rust-static)
  add_executable(
    test_effect_param_types_steps_rust
    test_effect_param_types_steps.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_effect_param_types_steps_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(
    test_effect_param_types_steps_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_effect_param_types_steps_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_effect_param_types_steps_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_param_types_steps_rust)
  add_test(test_effect_param_types_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_param_types_steps_rust)
endif()
