# Public API characterization: both C reference and hybrid libobs are selected
# by the existing Tier 2 harness. No shared native test graph registration.
add_executable(test_encoder_reroute test_encoder_reroute.c)
target_include_directories(test_encoder_reroute PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_encoder_reroute PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_encoder_reroute PRIVATE OBS::w32-pthreads)
endif()
add_test(NAME test_encoder_reroute COMMAND test_encoder_reroute)
set_tests_properties(test_encoder_reroute PROPERTIES TIMEOUT 30)
add_dependencies(cmocka-tests test_encoder_reroute)
