# Characterization of libobs/callback/decl.c before its port (#1438).
add_executable(test_decl test_decl.c)
target_include_directories(test_decl PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_decl PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_decl)
add_test(test_decl ${CMAKE_CURRENT_BINARY_DIR}/test_decl)
