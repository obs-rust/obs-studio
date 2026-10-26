# The vendored libnsgif decoded directly, without libobs: frame 0 of each GIF
# against its reference PPM (#2465). A one-frame GIF goes to FFmpeg in
# gs_image_file, so test_image_file does not reach libnsgif for it.
# test_nsgif_decode is defined in ../build/test-graph.json.

# The same cases against the Rust LZW decoder (#2466), block parsers (#2488)
# and frame decode (#2501), built and run only with the hybrid backend
# (OBS_LIBOBS_BACKEND=C has no Rust target). lzw_*, nsgif_blocks_* and
# nsgif_frame_* are hidden in the shared library, so the test takes them from
# the production staticlib at executable link time and compiles libnsgif's
# gif.c itself, without lzw.c, gif-blocks.c and gif-frame.c.
# The staticlib's obs-graphics also holds gs_image_file_ex_*, gs_texrender_* and
# the effect parser lifecycle, which call the private obs-graphics and effect
# parser adapters; libobs links those but does not export them, so this
# executable compiles its own copies, as test_sprite_geometry_steps_rust does.
# libnsgif's gif.c comes from test_nsgif_decode.c itself.
if(TARGET libobs_rust-static)
  add_executable(
    test_nsgif_decode_rust
    test_nsgif_decode.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_compile_definitions(
    test_nsgif_decode_rust
    PRIVATE NSGIF_TEST_RUST NSGIF_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/data"
  )
  target_include_directories(
    test_nsgif_decode_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(test_nsgif_decode_rust PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static)
  if(NOT MSVC)
    target_link_libraries(test_nsgif_decode_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_nsgif_decode_rust "address,undefined")
  add_dependencies(cmocka-tests test_nsgif_decode_rust)
  add_test(test_nsgif_decode_rust ${CMAKE_CURRENT_BINARY_DIR}/test_nsgif_decode_rust)
endif()
