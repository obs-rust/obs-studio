# Real public libobs matrix calls on a fabricated graphics context; no GPU.
add_executable(test_matrix_entrypoints test_matrix_entrypoints.c)
target_include_directories(test_matrix_entrypoints PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_matrix_entrypoints PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_matrix_entrypoints PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_matrix_entrypoints)
add_test(NAME test_matrix_entrypoints COMMAND test_matrix_entrypoints)
