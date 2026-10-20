# Characterization of the header-inline util/deque.h before its Rust
# equivalent in obs-util (#1546). The functions are static inline, so the test
# exercises the C header in both libobs builds; it links libobs for bmem.
add_executable(test_deque test_deque.c)
target_include_directories(test_deque PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_deque PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_deque)
add_test(test_deque ${CMAKE_CURRENT_BINARY_DIR}/test_deque)
