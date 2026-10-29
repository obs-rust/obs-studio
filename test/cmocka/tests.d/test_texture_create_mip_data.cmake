# Characterization of the mipmap rules of gs_texture_create() and
# gs_cubetexture_create(), with the data each passes on (#2859). A fake device
# in the test records the calls; no GPU, no display.
add_executable(test_texture_create_mip_data test_texture_create_mip_data.c)
target_include_directories(test_texture_create_mip_data PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_texture_create_mip_data PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_texture_create_mip_data)
add_test(test_texture_create_mip_data ${CMAKE_CURRENT_BINARY_DIR}/test_texture_create_mip_data)
