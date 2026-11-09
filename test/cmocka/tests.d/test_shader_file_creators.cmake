# Public C API characterization before the shader-file wrapper port (#4304).
# A fake graphics device needs no GPU or display. Run unchanged against both
# hybrid libobs and the explicit C reference in the Tier 2 harness.
add_executable(test_shader_file_creators test_shader_file_creators.c)
target_include_directories(test_shader_file_creators PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_shader_file_creators PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_shader_file_creators PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_shader_file_creators)
add_test(NAME test_shader_file_creators COMMAND test_shader_file_creators)
set_tests_properties(test_shader_file_creators PROPERTIES TIMEOUT 15)
