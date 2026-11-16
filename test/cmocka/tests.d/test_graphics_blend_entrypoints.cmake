# Actual-library characterization; retain this test unchanged through the Rust swap.
add_executable(test_graphics_blend_entrypoints test_graphics_blend_entrypoints.c)
target_link_libraries(test_graphics_blend_entrypoints PRIVATE OBS::libobs OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_graphics_blend_entrypoints PRIVATE OBS::w32-pthreads)
endif()
add_test(NAME test_graphics_blend_entrypoints COMMAND test_graphics_blend_entrypoints)
set_tests_properties(test_graphics_blend_entrypoints PROPERTIES LABELS "graphics")
add_dependencies(cmocka-tests test_graphics_blend_entrypoints)
