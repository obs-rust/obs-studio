# Exercise public shader introspection with a recording backend, no GPU.
add_executable(test_shader_param_introspection test_shader_param_introspection.c)
target_include_directories(test_shader_param_introspection PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_shader_param_introspection
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES}
)
if(OS_WINDOWS)
  target_link_libraries(test_shader_param_introspection PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_shader_param_introspection)
add_test(NAME test_shader_param_introspection COMMAND test_shader_param_introspection)
