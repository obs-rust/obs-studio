# Hidden-source characterization only; public caller coverage tests both backends.
add_executable(test_module_load_metadata test_module_load_metadata.c)
target_link_libraries(test_module_load_metadata PRIVATE OBS::libobs OBS::native-test-cmocka OBS::caption Uthash::Uthash)
target_include_directories(test_module_load_metadata PRIVATE "${CMAKE_SOURCE_DIR}")
if(OS_WINDOWS)
  target_link_libraries(test_module_load_metadata PRIVATE OBS::w32-pthreads)
endif()
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/module-load-metadata")
add_test(
  NAME test_module_load_metadata
  COMMAND test_module_load_metadata "${CMAKE_CURRENT_BINARY_DIR}/module-load-metadata"
)
add_dependencies(cmocka-tests test_module_load_metadata)
