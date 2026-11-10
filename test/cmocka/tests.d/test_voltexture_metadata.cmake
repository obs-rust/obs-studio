# Real public libobs wrappers with fake graphics backend callbacks; no GPU.
add_executable(test_voltexture_metadata test_voltexture_metadata.c)
target_include_directories(test_voltexture_metadata PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_voltexture_metadata PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_voltexture_metadata PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_voltexture_metadata)
add_test(NAME test_voltexture_metadata COMMAND test_voltexture_metadata)
