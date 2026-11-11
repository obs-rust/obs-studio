# Real public libobs wrappers with fake graphics backend callbacks; no GPU.
add_executable(test_cubetexture_access test_cubetexture_access.c)
target_include_directories(test_cubetexture_access PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_cubetexture_access PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_cubetexture_access PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_cubetexture_access)
add_test(NAME test_cubetexture_access COMMAND test_cubetexture_access)
