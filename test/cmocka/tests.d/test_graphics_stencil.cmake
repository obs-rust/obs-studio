# Recording-device characterization of public stencil dispatch (#4418).
add_executable(test_graphics_stencil test_graphics_stencil.c)
target_include_directories(test_graphics_stencil PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_graphics_stencil PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_graphics_stencil PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_graphics_stencil)
add_test(NAME test_graphics_stencil COMMAND test_graphics_stencil)
