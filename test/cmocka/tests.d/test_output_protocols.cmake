# Characterization of the output protocol lookups before the protocol-list
# match of libobs/obs-output.c is ported: the registered protocols, the
# registered check, and obs_enum_output_types_with_protocol() over service
# outputs with lists such as "RTMP;RTMPS", ";RTMP;;", "RTMP;RTMP", "RTMPX" and
# "HLS;" (order, duplicates, early stop, whole names, case). Public API only,
# so it also covers the hybrid libobs. obs_startup() needs a display on
# Linux, so the test runs under xvfb-run when it is installed and the cases
# skip without a display.
add_executable(test_output_protocols test_output_protocols.c)
target_include_directories(test_output_protocols PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_output_protocols PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_output_protocols)
find_program(XVFB_RUN xvfb-run)
if(OS_LINUX AND XVFB_RUN)
  add_test(
    NAME test_output_protocols
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_output_protocols>
  )
else()
  add_test(test_output_protocols ${CMAKE_CURRENT_BINARY_DIR}/test_output_protocols)
endif()
