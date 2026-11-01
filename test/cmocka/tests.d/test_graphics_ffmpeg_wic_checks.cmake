# Characterization of two checks of the image loader's Windows host
# (libobs/graphics/graphics-ffmpeg-host-windows.c): the file-name check of
# gs_image_host_fallback_load() and the pixel-format pick of
# wic_image_init_internal(), moved to
# libobs/graphics/graphics-ffmpeg-host-wic-checks.c before their port to
# rust/obs-graphics (image_pq_scrgb, #3357). Names of exactly 4 bytes, longer
# names (characterized, not endorsed: never refused), every (scrgb, pq10)
# pair and the enum's values. Builds the moved file beside it: no Windows
# header and no libobs core, so it runs on every platform. Target and test
# are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# includes only graphics-ffmpeg-host-wic-checks.h, so without the C file
# beside it it links gs_image_host_fallback_refuses() and gs_image_wic_kind()
# from the archive, on every platform although only Windows calls them.
# Linking the archive directly avoids exporting hidden symbols just for a
# test; its obs-graphics objects need libobs and the two graphics adapters,
# as test_graphics_ffmpeg_pq_steps_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_graphics_ffmpeg_wic_checks_rust
    test_graphics_ffmpeg_wic_checks.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_graphics_ffmpeg_wic_checks_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(
    test_graphics_ffmpeg_wic_checks_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_graphics_ffmpeg_wic_checks_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_graphics_ffmpeg_wic_checks_rust "address,undefined")
  add_dependencies(cmocka-tests test_graphics_ffmpeg_wic_checks_rust)
  add_test(test_graphics_ffmpeg_wic_checks_rust ${CMAKE_CURRENT_BINARY_DIR}/test_graphics_ffmpeg_wic_checks_rust)
endif()
