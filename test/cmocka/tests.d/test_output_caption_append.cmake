# Direct C queue characterization; kept separate from the Rust swap.
add_executable(test_output_caption_append test_output_caption_append.c)
target_include_directories(test_output_caption_append PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
target_link_libraries(
  test_output_caption_append
  PRIVATE OBS::libobs OBS::caption OBS::native-test-cmocka $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_test(NAME test_output_caption_append COMMAND test_output_caption_append)
set_target_properties(test_output_caption_append PROPERTIES FOLDER "tests/cmocka")
add_dependencies(cmocka-tests test_output_caption_append)
