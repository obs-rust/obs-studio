# The output stream delay end to end through the libobs that ships, which
# test_output_delay (it compiles obs-output-delay.c.h into itself) does not
# reach: a registered audio encoder and encoded output, a real-time audio
# thread, and one-second delays, so the cases take about 13 s in all.
# obs_startup needs an X display on Linux.
add_executable(test_output_delay_e2e test_output_delay_e2e.c)
target_include_directories(test_output_delay_e2e PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_output_delay_e2e PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_output_delay_e2e)
find_program(XVFB_RUN xvfb-run)
if(XVFB_RUN)
  add_test(test_output_delay_e2e ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_output_delay_e2e)
else()
  add_test(test_output_delay_e2e ${CMAKE_CURRENT_BINARY_DIR}/test_output_delay_e2e)
endif()
set_tests_properties(test_output_delay_e2e PROPERTIES TIMEOUT 120)
