# Characterization of the pixel route of libobs/graphics/graphics-ffmpeg.c
# (ffmpeg_image_reformat_frame()'s choice between the straight copy, the
# premultiplied rows, the 16-bit rows and swscale, the format swscale is given
# and its range, the format afterwards, convert_format() and the straight row
# copy), moved to libobs/graphics/graphics-ffmpeg-route.c before its port to
# rust/obs-graphics (image_route, #3396). A source line size shorter than
# cx * 4 leaves the end of each row unwritten: characterized, not endorsed.
# Builds the moved file on its own: no FFmpeg header and no libobs core, so it
# runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source (test_graphics_ffmpeg_route_steps_rust, below,
# runs the same test against the Rust shims). test_image_file and
# the other image tests cover the caller through the real libobs. Target and
# test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims: the test
# calls only the three functions through graphics-ffmpeg-route.h, so this
# variant builds the same source without the moved file and links
# gs_ffmpeg_image_plan(), gs_ffmpeg_image_color_format() and
# gs_ffmpeg_image_copy_rows() from the archive. Linking the archive directly
# avoids exporting hidden symbols just for a test; its obs-graphics objects
# need libobs and the two graphics adapters, as
# test_graphics_ffmpeg_pq_steps_rust does.
if(TARGET libobs_rust-static)
  add_executable(
    test_graphics_ffmpeg_route_steps_rust
    test_graphics_ffmpeg_route_steps.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/graphics/obs-graphics-adapter.c.h"
  )
  target_include_directories(
    test_graphics_ffmpeg_route_steps_rust
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
  )
  target_link_libraries(
    test_graphics_ffmpeg_route_steps_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  if(NOT MSVC)
    target_link_libraries(test_graphics_ffmpeg_route_steps_rust PRIVATE m)
  endif()
  obs_test_sanitize(test_graphics_ffmpeg_route_steps_rust "address,undefined")
  add_dependencies(cmocka-tests test_graphics_ffmpeg_route_steps_rust)
  add_test(test_graphics_ffmpeg_route_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_graphics_ffmpeg_route_steps_rust)
endif()
