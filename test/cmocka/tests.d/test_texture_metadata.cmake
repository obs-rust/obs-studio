# Real public libobs wrappers with fake graphics backend callbacks; no GPU.
add_executable(test_texture_metadata test_texture_metadata.c)
target_include_directories(test_texture_metadata PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_texture_metadata PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_texture_metadata PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_texture_metadata)
add_test(NAME test_texture_metadata COMMAND test_texture_metadata)
