# Characterization of the util/darray.h operations the output interleaver uses
# (insert, erase, erase range, pop front, push back new), before the Rust
# equivalent in obs-util gains them (#1594). The functions are static inline,
# so the test exercises the C header in both libobs builds; it links libobs for
# bmem.
add_executable(test_darray_ops test_darray_ops.c)
target_include_directories(test_darray_ops PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_darray_ops PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_darray_ops)
add_test(test_darray_ops ${CMAKE_CURRENT_BINARY_DIR}/test_darray_ops)
