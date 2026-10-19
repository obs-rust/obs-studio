# Characterization of libobs/callback/signal.c.h before its port (#1441).
add_executable(test_signal test_signal.c)
target_include_directories(test_signal PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_signal PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_signal)
add_test(test_signal ${CMAKE_CURRENT_BINARY_DIR}/test_signal)
