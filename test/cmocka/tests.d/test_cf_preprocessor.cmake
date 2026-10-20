# Characterization of the cf preprocessor in libobs/util/cf-lexer.c.h before its port (#1499).
add_executable(test_cf_preprocessor test_cf_preprocessor.c)
target_include_directories(test_cf_preprocessor PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_cf_preprocessor PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_cf_preprocessor)
add_test(test_cf_preprocessor ${CMAKE_CURRENT_BINARY_DIR}/test_cf_preprocessor)
