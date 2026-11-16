# Real libobs entry points with fake backend callbacks; no display or GPU.
add_executable(test_texture_planar_creators test_texture_planar_creators.c)
target_include_directories(test_texture_planar_creators PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_texture_planar_creators PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_texture_planar_creators PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_texture_planar_creators)
add_test(NAME test_texture_planar_creators COMMAND test_texture_planar_creators)
