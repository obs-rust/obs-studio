# The immediate-mode vertex builder of graphics.c through libobs's exports
# (#3132): a draw in each mode, the immediate cap, and the stop and save with
# no vertex, which leave no vertex buffer data behind and free every array. A
# fake device in the test owns the vertex buffers; no GPU, no display. ASan
# and LeakSanitizer check libobs's allocations.
add_executable(test_graphics_immediate_render test_graphics_immediate_render.c)
target_include_directories(test_graphics_immediate_render PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_graphics_immediate_render PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_graphics_immediate_render "address,undefined")

add_dependencies(cmocka-tests test_graphics_immediate_render)
add_test(test_graphics_immediate_render ${CMAKE_CURRENT_BINARY_DIR}/test_graphics_immediate_render)
