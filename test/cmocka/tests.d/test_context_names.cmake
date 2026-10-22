# Characterization of the context names in libobs/obs.c before their port:
# the " 2", " 3", ... suffix of a duplicate name on insert and rename, and
# the "__unnamed%04lld" name of a NULL or empty one. Public API only, so it
# also covers the hybrid libobs. Needs no video; obs_startup() needs a display
# on Linux, so the test runs under xvfb-run when it is installed and the cases
# skip without a display. Each case starts its own libobs, so Xvfb runs with
# -noreset: a reset after the last client left refused the next one.
add_executable(test_context_names test_context_names.c)
target_include_directories(test_context_names PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_context_names PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_context_names)
find_program(XVFB_RUN xvfb-run)
if(OS_LINUX AND XVFB_RUN)
  add_test(
    NAME test_context_names
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_context_names>
  )
else()
  add_test(test_context_names ${CMAKE_CURRENT_BINARY_DIR}/test_context_names)
endif()
