# Characterization of the JPEG XR PQ-to-scRGB conversion of
# libobs/graphics/graphics-ffmpeg-host-windows.c (pq_to_linear() and
# convert_pq_to_cccs()), moved to libobs/graphics/graphics-ffmpeg-host-pq.c
# before its port to rust/obs-graphics (image_pq_scrgb, #3250): the inverse
# PQ curve at code values 0, 1, 2, 512, 1022 and 1023, half.h's
# half_from_float() at its edges, black, white, a grey ramp, each primary at
# full scale and at code value 1, bits 30-31, the texel order with alpha 1.0
# and an empty input. A texel outside BT.709 keeps its negative values:
# characterized, not endorsed. Values that depend on powf() are checked to
# within the last place. Builds the moved file on its own: no Windows header
# and no libobs core, so it runs on every platform. The function is hidden in
# libobs and built only on Windows, so this characterizes that C source; no
# real-libobs test loads a JPEG XR file. Target and test are declared in
# ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shim. With
# GS_PQ_STEPS_TEST_RUST the test includes only graphics-ffmpeg-host-pq.h and
# half.h, and links gs_image_pq10_to_scrgb() from the archive, so it runs on
# every platform although only Windows calls the conversion. Linking the
# archive directly avoids exporting a hidden symbol just for a test; its
# obs-graphics objects need libobs and the two graphics adapters, as
# test_graphics_immediate_steps_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_graphics_ffmpeg_pq_steps_rust
    test_graphics_ffmpeg_pq_steps.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_compile_definitions(test_graphics_ffmpeg_pq_steps_rust PRIVATE GS_PQ_STEPS_TEST_RUST)
  target_include_directories(
    test_graphics_ffmpeg_pq_steps_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(
    test_graphics_ffmpeg_pq_steps_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_graphics_ffmpeg_pq_steps_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_graphics_ffmpeg_pq_steps_rust "address,undefined")
  add_dependencies(cmocka-tests test_graphics_ffmpeg_pq_steps_rust)
  add_test(test_graphics_ffmpeg_pq_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_graphics_ffmpeg_pq_steps_rust)
endif()
