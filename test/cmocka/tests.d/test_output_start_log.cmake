# A start of an output that the frontend refuses while the outputs are
# disabled must be logged (obs-rust/obs-studio#817). Builds the Qt-free header
# the OBSBasic start functions use.
add_executable(test_output_start_log test_output_start_log.cpp)
target_include_directories(test_output_start_log PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_compile_features(test_output_start_log PRIVATE cxx_std_17)
target_link_libraries(test_output_start_log PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_output_start_log)
add_test(test_output_start_log ${CMAKE_CURRENT_BINARY_DIR}/test_output_start_log)
