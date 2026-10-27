# The state stacks of the graphics context on plain darrays (#2517).
# Defined in ../build/test-graph.json.

# The same cases against the Rust shims (GS_STATE_STACKS_TEST_RUST), built and
# run only with the hybrid backend (OBS_LIBOBS_BACKEND=C has no Rust target).
# The gs_*_stack_* functions are hidden in the shared library, so the test
# selects them from the exact production staticlib at executable link time.
# The staticlib's obs-graphics also holds gs_image_file_ex_*, gs_texrender_*
# and the effect parser lifecycle, which call libobs exports (linked), the
# vendored libnsgif and the private obs-graphics and effect parser adapters;
# libobs links those but does not export them, so this executable compiles its
# own copies, as test_sprite_geometry_steps_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_graphics_state_stacks_rust
    test_graphics_state_stacks.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/build/libobs.nsgif+.h.c"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_compile_definitions(test_graphics_state_stacks_rust PRIVATE GS_STATE_STACKS_TEST_RUST)
  target_include_directories(
    test_graphics_state_stacks_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  if(MSVC)
    set_source_files_properties(
      "${CMAKE_SOURCE_DIR}/libobs/build/libobs.nsgif+.h.c"
      TARGET_DIRECTORY test_graphics_state_stacks_rust
      PROPERTIES COMPILE_OPTIONS "/wd4244;/wd4267"
    )
  endif()
  target_link_libraries(
    test_graphics_state_stacks_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(MSVC)
    # The staticlib object that holds the shims also defines the matrix4_*
    # functions the test calls, which obs.lib provides as well (LNK2005,
    # #2533); allow the duplicate, which is the same code. LNK4088 (/FORCE)
    # cannot be ignored, so this one link runs without /WX. Listing the
    # staticlib first instead pulls obs_util objects that need libobs internals.
    target_link_options(
      test_graphics_state_stacks_rust
      PRIVATE /FORCE:MULTIPLE /INCREMENTAL:NO /WX:NO /IGNORE:4006 /IGNORE:4075
    )
  else()
    target_link_libraries(test_graphics_state_stacks_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_graphics_state_stacks_rust "address,undefined")
  add_dependencies(cmocka-tests test_graphics_state_stacks_rust)
  add_test(test_graphics_state_stacks_rust ${CMAKE_CURRENT_BINARY_DIR}/test_graphics_state_stacks_rust)
endif()
